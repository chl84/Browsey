//! Bounded, exact-path checkpoints for generated native transfer fixtures only.
use super::check_path;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum FaultKind {
    NoSpace,
    Unavailable,
    Transient,
    CloudQuota,
    CloudRateLimited,
    CloudAuthRequired,
}

impl FaultKind {
    fn cloud(self) -> bool {
        matches!(
            self,
            Self::CloudQuota | Self::CloudRateLimited | Self::CloudAuthRequired
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Plan {
    id: String,
    source: String,
    target: String,
    hold_phase: String,
    hold_bytes: u64,
    hold_ms: u64,
    slow_ms: u64,
    #[serde(default)]
    fault: Option<FaultKind>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    phase: String,
    bytes: u64,
    held: bool,
    checkpoints: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    fault_code: Option<String>,
}
struct Probe {
    plan: Plan,
    consumed: AtomicBool,
    fault_used: AtomicBool,
    observation: Mutex<Observation>,
}
static PROBES: OnceCell<Vec<Probe>> = OnceCell::new();

fn validate(roots: &[String], plans: Vec<Plan>) -> Result<Vec<Probe>, &'static str> {
    if plans.len() > 32 {
        return Err("Too many native transfer probes");
    }
    let mut ids = std::collections::HashSet::new();
    let mut pairs = std::collections::HashSet::new();
    for plan in &plans {
        if plan.id.is_empty()
            || plan.id.len() > 64
            || !plan
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !ids.insert(&plan.id)
            || !pairs.insert((&plan.source, &plan.target))
            || !matches!(
                plan.hold_phase.as_str(),
                "prepare" | "validation" | "start" | "written" | "finalize"
            )
            || plan.hold_bytes > 65536
            || plan.hold_ms > 5000
            || plan.slow_ms > 500
            || plan.fault.is_some_and(|fault| {
                if fault.cloud() {
                    plan.hold_phase != "validation"
                        || plan.hold_bytes != 0
                        || !plan.target.starts_with("rclone://")
                } else {
                    !matches!(plan.hold_phase.as_str(), "start" | "written")
                }
            })
        {
            return Err("Invalid bounded native transfer probe");
        }
        for raw in [&plan.source, &plan.target] {
            check_path(roots, raw)?;
            if roots.contains(raw) {
                return Err("Transfer probes require exact generated children");
            }
        }
        if plan.source == plan.target {
            return Err("Transfer probe paths must differ");
        }
    }
    Ok(plans
        .into_iter()
        .map(|plan| Probe {
            plan,
            consumed: AtomicBool::new(false),
            fault_used: AtomicBool::new(false),
            observation: Mutex::new(Observation::default()),
        })
        .collect())
}

pub(super) fn initialize(roots: &[String], plans: Vec<Plan>) -> Result<(), &'static str> {
    PROBES
        .set(validate(roots, plans)?)
        .map_err(|_| "Transfer probes already initialized")
}

pub(crate) fn selected(source: &str, target: &str) -> bool {
    PROBES.get().is_some_and(|probes| {
        probes
            .iter()
            .any(|p| p.plan.source == source && p.plan.target == target)
    })
}

fn observe(probe: &Probe, phase: &str, bytes: u64, abort: &impl Fn() -> bool) {
    let hold = phase == probe.plan.hold_phase
        && bytes >= probe.plan.hold_bytes
        && !probe.consumed.swap(true, Ordering::SeqCst);
    {
        let mut state = probe
            .observation
            .lock()
            .expect("native transfer probe lock");
        state.phase = phase.to_owned();
        state.bytes = bytes;
        state.held = hold;
        state.checkpoints += 1;
    }
    let ms = if hold {
        probe.plan.hold_ms
    } else if phase == "written" {
        probe.plan.slow_ms
    } else {
        0
    };
    let deadline = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < deadline && !abort() {
        std::thread::sleep(Duration::from_millis(20));
    }
    probe
        .observation
        .lock()
        .expect("native transfer probe lock")
        .held = false;
}

pub(crate) fn checkpoint(
    source: &str,
    target: &str,
    phase: &str,
    bytes: u64,
    abort: impl Fn() -> bool,
) {
    if let Some(probes) = PROBES.get() {
        if let Some(probe) = probes
            .iter()
            .find(|p| p.plan.source == source && p.plan.target == target)
        {
            observe(probe, phase, bytes, &abort);
        }
    }
}

// One-use I/O faults follow actual exact-path checkpoints; no filesystem or
// provider service is modified. This entire module is absent from production.
fn fault_for(probe: &Probe, phase: &str, bytes: u64) -> Option<std::io::Error> {
    let kind = probe.plan.fault?;
    if kind.cloud()
        || phase != probe.plan.hold_phase
        || bytes < probe.plan.hold_bytes
        || probe.fault_used.swap(true, Ordering::SeqCst)
    {
        return None;
    }
    Some(match kind {
        FaultKind::NoSpace => std::io::Error::from_raw_os_error(libc::ENOSPC),
        FaultKind::Unavailable => std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "Owned provider unavailable (native fault)",
        ),
        FaultKind::CloudQuota | FaultKind::CloudRateLimited | FaultKind::CloudAuthRequired => {
            return None
        }
        FaultKind::Transient => std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "Owned transient I/O failure (native fault)",
        ),
    })
}

fn cloud_fault_for(probe: &Probe) -> Option<crate::commands::cloud::CloudCommandError> {
    let kind = probe.plan.fault?;
    if !kind.cloud()
        || !probe.consumed.load(Ordering::SeqCst)
        || probe.fault_used.swap(true, Ordering::SeqCst)
    {
        return None;
    }
    let message = match kind {
        FaultKind::CloudQuota => "Quota exceeded (owned native provider fault)",
        FaultKind::CloudRateLimited => "Rate limit exceeded (owned native provider fault)",
        FaultKind::CloudAuthRequired => "Authentication failed (owned native provider fault)",
        _ => return None,
    };
    let code =
        crate::commands::cloud::providers::rclone::classify_rclone_failure_code(None, message);
    probe
        .observation
        .lock()
        .expect("native fault observation lock")
        .fault_code = Some(crate::errors::domain::ErrorCode::as_code_str(code).to_owned());
    Some(crate::commands::cloud::CloudCommandError::new(
        code, message,
    ))
}

pub(crate) fn cloud_fault(
    source: &str,
    target: &str,
) -> Option<crate::commands::cloud::CloudCommandError> {
    PROBES
        .get()?
        .iter()
        .find(|probe| probe.plan.source == source && probe.plan.target == target)
        .and_then(cloud_fault_for)
}

pub(crate) fn fault(source: &str, target: &str, phase: &str, bytes: u64) -> Option<std::io::Error> {
    PROBES
        .get()?
        .iter()
        .find(|p| p.plan.source == source && p.plan.target == target)
        .and_then(|p| fault_for(p, phase, bytes))
}

fn matches_preparation(probe: &Probe, directory: &str, event: &str) -> bool {
    ["mixed-copy-", "mixed-cut-", "cloud-copy-", "cloud-cut-"]
        .iter()
        .any(|prefix| event.starts_with(prefix))
        && probe.plan.hold_phase == "prepare"
        && probe
            .plan
            .target
            .rsplit_once('/')
            .is_some_and(|(parent, _)| parent == directory)
}

pub(crate) fn preparation_checkpoint(directory: &str, event: &str, abort: impl Fn() -> bool) {
    if let Some(probes) = PROBES.get() {
        if let Some(probe) = probes
            .iter()
            .find(|p| matches_preparation(p, directory, event))
        {
            observe(probe, "prepare", 0, &abort);
        }
    }
}

pub(super) fn status() -> serde_json::Value {
    serde_json::Value::Array(PROBES.get().into_iter().flatten().map(|probe| {
        serde_json::json!({"id":probe.plan.id, "consumed":probe.consumed.load(Ordering::SeqCst),
            "faultUses":usize::from(probe.fault_used.load(Ordering::SeqCst)),
            "observation":*probe.observation.lock().expect("native transfer probe lock")})
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Plan {
        serde_json::from_value(serde_json::json!({
            "id":"copy-fixture", "source":"/owned/files/a", "target":"/owned/files/b",
            "holdPhase":"written", "holdBytes":16384, "holdMs":0, "slowMs":0,
        }))
        .unwrap()
    }
    #[test]
    fn probes_reject_outside_paths_duplicate_targets_and_unbounded_delays() {
        let roots = vec!["/owned/files".to_owned()];
        assert!(validate(&roots, vec![plan()]).is_ok());
        assert!(validate(&roots, vec![plan(), plan()]).is_err());
        for source in [
            "/outside/file",
            "/owned/files",
            "/owned/files/../other",
            "/owned/files2/a",
        ] {
            let mut p = plan();
            p.source = source.to_owned();
            assert!(validate(&roots, vec![p]).is_err());
        }
        let mut p = plan();
        p.hold_ms = 5001;
        assert!(validate(&roots, vec![p]).is_err());
        let mut p = plan();
        p.hold_bytes = 65537;
        assert!(validate(&roots, vec![p]).is_err());
    }
    #[test]
    fn faults_require_actual_boundary_are_one_use_and_reject_finalize_or_prepare() {
        let roots = ["/owned/files".to_owned()];
        let mut p = plan();
        p.fault = Some(FaultKind::NoSpace);
        let probe = validate(&roots, vec![p]).unwrap().remove(0);
        assert!(fault_for(&probe, "start", 0).is_none());
        assert!(fault_for(&probe, "written", 8192).is_none());
        assert_eq!(
            fault_for(&probe, "written", 16384).unwrap().raw_os_error(),
            Some(libc::ENOSPC)
        );
        assert!(fault_for(&probe, "written", 32768).is_none());
        for phase in ["prepare", "finalize", "validation"] {
            let mut p = plan();
            p.fault = Some(FaultKind::Transient);
            p.hold_phase = phase.to_owned();
            assert!(validate(&roots, vec![p]).is_err());
        }
    }
    #[test]
    fn preparation_delay_ignores_navigation_and_other_owned_directories() {
        let mut p = plan();
        p.hold_phase = "prepare".to_owned();
        let probe = validate(&["/owned/files".to_owned()], vec![p])
            .unwrap()
            .remove(0);
        for event in ["mixed-copy-1", "mixed-cut-1", "cloud-copy-1", "cloud-cut-1"] {
            assert!(matches_preparation(&probe, "/owned/files", event));
            assert!(!matches_preparation(
                &probe,
                "/owned/files/elsewhere",
                event
            ));
        }
        for event in [
            "cloud-list-1",
            "cloud-probe-1",
            "cloud-",
            "mixed-list-1",
            "",
        ] {
            assert!(!matches_preparation(&probe, "/owned/files", event));
        }
    }
    #[test]
    fn checkpoint_requires_the_declared_phase_and_actual_byte_boundary() {
        let probe = validate(&["/owned/files".to_owned()], vec![plan()])
            .unwrap()
            .remove(0);
        observe(&probe, "start", 0, &|| false);
        observe(&probe, "written", 8192, &|| false);
        assert!(!probe.consumed.load(Ordering::SeqCst));
        observe(&probe, "written", 16384, &|| false);
        assert!(probe.consumed.load(Ordering::SeqCst));
        observe(&probe, "written", 32768, &|| false);
        assert_eq!(probe.observation.lock().unwrap().bytes, 32768);
        assert!(!probe.observation.lock().unwrap().held);
    }
    #[test]
    fn cloud_faults_are_exact_owned_prewrite_once_and_use_real_error_classification() {
        let roots = vec!["/owned".to_owned(), "rclone://Test/owned".to_owned()];
        for (fault, expected) in [
            (
                "cloud-quota",
                crate::commands::cloud::CloudCommandErrorCode::RateLimited,
            ),
            (
                "cloud-rate-limited",
                crate::commands::cloud::CloudCommandErrorCode::RateLimited,
            ),
            (
                "cloud-auth-required",
                crate::commands::cloud::CloudCommandErrorCode::AuthRequired,
            ),
        ] {
            let mut value = serde_json::json!({"id":"cloud-error","source":"/owned/source","target":"rclone://Test/owned/target","holdPhase":"validation","holdBytes":0,"holdMs":0,"slowMs":0,"fault":fault});
            let probes =
                validate(&roots, vec![serde_json::from_value(value.clone()).unwrap()]).unwrap();
            let probe = &probes[0];
            assert!(cloud_fault_for(probe).is_none());
            observe(probe, "validation", 0, &|| false);
            assert_eq!(cloud_fault_for(probe).unwrap().code(), expected);
            assert!(cloud_fault_for(probe).is_none());
            value["target"] = serde_json::json!("/owned/local-target");
            assert!(
                validate(&roots, vec![serde_json::from_value(value.clone()).unwrap()]).is_err()
            );
            value["target"] = serde_json::json!("rclone://Test/owned/target");
            value["holdPhase"] = serde_json::json!("written");
            assert!(
                validate(&roots, vec![serde_json::from_value(value.clone()).unwrap()]).is_err()
            );
            value["holdPhase"] = serde_json::json!("validation");
            value["source"] = serde_json::json!("/outside/file");
            assert!(validate(&roots, vec![serde_json::from_value(value).unwrap()]).is_err());
        }
    }
}

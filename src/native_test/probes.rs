//! Bounded, exact-path checkpoints for generated native transfer fixtures only.
use super::check_path;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

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
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    phase: String,
    bytes: u64,
    held: bool,
    checkpoints: usize,
}
struct Probe {
    plan: Plan,
    consumed: AtomicBool,
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
                "validation" | "start" | "written" | "finalize"
            )
            || plan.hold_bytes > 65536
            || plan.hold_ms > 5000
            || plan.slow_ms > 500
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

pub(super) fn status() -> serde_json::Value {
    serde_json::Value::Array(PROBES.get().into_iter().flatten().map(|probe| {
        serde_json::json!({"id":probe.plan.id, "consumed":probe.consumed.load(Ordering::SeqCst),
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
}

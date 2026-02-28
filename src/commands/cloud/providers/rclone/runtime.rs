use super::{
    error::map_rclone_error,
    parse::{parse_rclone_version_stdout, parse_rclone_version_triplet},
    CloudCommandError, CloudCommandErrorCode, CloudCommandResult, RcloneCli, RcloneCloudProvider,
    RcloneCommandSpec, RcloneSubcommand,
};
use std::{
    collections::HashMap,
    ffi::OsString,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tracing::debug;

const MIN_RCLONE_VERSION: (u64, u64, u64) = (1, 67, 0);
#[cfg(not(test))]
pub(super) const RCLONE_RUNTIME_PROBE_FAILURE_RETRY_BACKOFF: Duration = Duration::from_secs(5);
#[cfg(test)]
pub(super) const RCLONE_RUNTIME_PROBE_FAILURE_RETRY_BACKOFF: Duration = Duration::from_millis(1);

#[derive(Debug, Clone)]
enum RuntimeProbeCacheEntry {
    Ready,
    Failed {
        error: CloudCommandError,
        retry_after: Instant,
    },
}

type RuntimeProbeCache = HashMap<OsString, RuntimeProbeCacheEntry>;

static RCLONE_RUNTIME_PROBE: OnceLock<Mutex<RuntimeProbeCache>> = OnceLock::new();

fn runtime_probe_cache() -> &'static Mutex<RuntimeProbeCache> {
    RCLONE_RUNTIME_PROBE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
pub(super) fn reset_runtime_probe_cache_for_tests() {
    let mut cache = match runtime_probe_cache().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.clear();
}

impl RcloneCloudProvider {
    pub(super) fn ensure_runtime_ready(&self) -> CloudCommandResult<()> {
        let binary = self.cli().binary().to_os_string();
        let now = Instant::now();
        {
            let cache = match runtime_probe_cache().lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(entry) = cache.get(&binary) {
                match entry {
                    RuntimeProbeCacheEntry::Ready => return Ok(()),
                    RuntimeProbeCacheEntry::Failed { error, retry_after } if *retry_after > now => {
                        return Err(error.clone());
                    }
                    RuntimeProbeCacheEntry::Failed { .. } => {}
                }
            }
        }

        let probe_result = probe_rclone_runtime(self.cli());
        let mut cache = match runtime_probe_cache().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match &probe_result {
            Ok(()) => {
                cache.insert(binary, RuntimeProbeCacheEntry::Ready);
            }
            Err(error) => {
                cache.insert(
                    binary,
                    RuntimeProbeCacheEntry::Failed {
                        error: error.clone(),
                        retry_after: Instant::now() + RCLONE_RUNTIME_PROBE_FAILURE_RETRY_BACKOFF,
                    },
                );
            }
        }
        probe_result
    }
}

fn probe_rclone_runtime(cli: &RcloneCli) -> CloudCommandResult<()> {
    let output = cli
        .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::Version))
        .map_err(map_rclone_error)?;
    let version = parse_rclone_version_stdout(&output.stdout).ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            "Unexpected `rclone version` output; cannot verify rclone runtime",
        )
    })?;
    let numeric = parse_rclone_version_triplet(&version).ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!("Unsupported rclone version format: {version}"),
        )
    })?;
    if numeric < MIN_RCLONE_VERSION {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!(
                "rclone v{version} is too old; Browsey requires rclone v{}.{}.{} or newer",
                MIN_RCLONE_VERSION.0, MIN_RCLONE_VERSION.1, MIN_RCLONE_VERSION.2
            ),
        ));
    }
    debug!(version = %version, "rclone runtime probe succeeded");
    Ok(())
}

use super::{RcloneCliError, RcloneRcClient, RcloneRcMethod, RcloneSubcommand};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
#[cfg(test)]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::{io, sync::atomic::AtomicBool, time::Instant};
use tracing::warn;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForcedAsyncStatusErrorMode {
    CopyFile,
    DeleteFile,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(super) struct ForcedAsyncStatusErrorState {
    mode: ForcedAsyncStatusErrorMode,
    job_id: u64,
    status_error_kind: io::ErrorKind,
    job_stop_calls: Arc<AtomicUsize>,
}

impl RcloneRcClient {
    #[cfg(test)]
    pub fn with_forced_async_status_error_on_copy_for_tests(
        mut self,
        status_error_kind: io::ErrorKind,
    ) -> Self {
        self.forced_async_status_error = Some(ForcedAsyncStatusErrorState {
            mode: ForcedAsyncStatusErrorMode::CopyFile,
            job_id: 9101,
            status_error_kind,
            job_stop_calls: Arc::new(AtomicUsize::new(0)),
        });
        self
    }

    #[cfg(test)]
    pub fn with_forced_async_status_error_on_delete_for_tests(
        mut self,
        status_error_kind: io::ErrorKind,
    ) -> Self {
        self.forced_async_status_error = Some(ForcedAsyncStatusErrorState {
            mode: ForcedAsyncStatusErrorMode::DeleteFile,
            job_id: 9102,
            status_error_kind,
            job_stop_calls: Arc::new(AtomicUsize::new(0)),
        });
        self
    }

    #[cfg(test)]
    pub fn forced_job_stop_calls_for_tests(&self) -> usize {
        self.forced_async_status_error
            .as_ref()
            .map(|state| state.job_stop_calls.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    pub(super) fn run_method_async_if_cancelable(
        &self,
        method: RcloneRcMethod,
        payload: Value,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        if cancel_token.is_none() {
            return self.run_method(method, payload);
        }
        self.run_method_async_with_job_control(method, payload, cancel_token)
    }

    fn run_method_async_with_job_control(
        &self,
        method: RcloneRcMethod,
        payload: Value,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        self.run_method_async_with_job_control_and_progress(
            method,
            payload,
            None,
            cancel_token,
            |_| {},
        )
    }

    pub(super) fn run_method_async_with_job_control_and_progress<F>(
        &self,
        method: RcloneRcMethod,
        mut payload: Value,
        group: Option<&str>,
        cancel_token: Option<&AtomicBool>,
        mut on_progress: F,
    ) -> Result<Value, RcloneCliError>
    where
        F: FnMut(Value),
    {
        let Some(payload_obj) = payload.as_object_mut() else {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "rclone rc {} async payload must be a JSON object",
                method.as_str()
            ))));
        };
        payload_obj.insert("_async".to_string(), Value::Bool(true));
        if let Some(group) = group {
            payload_obj.insert("_group".to_string(), Value::String(group.to_string()));
        }

        let kickoff = self.run_method(method, payload)?;
        let job_id = kickoff
            .get("jobid")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RcloneCliError::Io(io::Error::other(format!(
                    "rclone rc {} async response missing numeric `jobid`",
                    method.as_str()
                )))
            })?;

        let total_timeout = super::async_method_total_timeout(method);
        let deadline = Instant::now() + total_timeout;

        loop {
            if is_cancelled(cancel_token) {
                if let Err(error) = self.job_stop(job_id) {
                    warn!(
                        method = method.as_str(),
                        job_id,
                        error = %error,
                        "failed to stop cancelled rclone rc job"
                    );
                }
                return Err(RcloneCliError::Cancelled {
                    subcommand: RcloneSubcommand::Rc,
                });
            }

            if let Some(group) = group {
                if let Ok(stats) = self.core_stats(Some(group), true) {
                    on_progress(stats);
                }
            }

            let status = match self.job_status(job_id) {
                Ok(status) => status,
                Err(error) => {
                    if let Err(stop_error) = self.job_stop(job_id) {
                        warn!(
                            method = method.as_str(),
                            job_id,
                            status_error = %error,
                            stop_error = %stop_error,
                            "failed to stop async rclone rc job after status polling error"
                        );
                    }
                    return Err(RcloneCliError::AsyncJobStateUnknown {
                        subcommand: RcloneSubcommand::Rc,
                        operation: method.as_str().to_string(),
                        job_id,
                        reason: error.to_string(),
                    });
                }
            };
            let finished = status
                .get("finished")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if finished {
                if let Some(group) = group {
                    if let Ok(stats) = self.core_stats(Some(group), true) {
                        on_progress(stats);
                    }
                }
                let success = status
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if success {
                    return Ok(status);
                }
                let message = status
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|msg| !msg.is_empty())
                    .unwrap_or("rclone rc async job failed");
                return Err(RcloneCliError::Io(io::Error::other(format!(
                    "rclone rc {} async job {job_id} failed: {message}",
                    method.as_str()
                ))));
            }

            if Instant::now() >= deadline {
                if let Err(error) = self.job_stop(job_id) {
                    warn!(
                        method = method.as_str(),
                        job_id,
                        error = %error,
                        "failed to stop timed-out rclone rc job"
                    );
                }
                return Err(RcloneCliError::Timeout {
                    subcommand: RcloneSubcommand::Rc,
                    timeout: total_timeout,
                    stdout: String::new(),
                    stderr: format!("rclone rc {} async job {job_id} timed out", method.as_str()),
                });
            }

            std::thread::sleep(super::RCLONE_RC_ASYNC_POLL_SLICE);
        }
    }

    #[cfg(test)]
    pub(super) fn run_method_forced_async_status_error_for_tests(
        &self,
        method: RcloneRcMethod,
        payload: &Value,
    ) -> Option<Result<Value, RcloneCliError>> {
        let state = self.forced_async_status_error.as_ref()?;
        match method {
            RcloneRcMethod::OperationsCopyFile
                if state.mode == ForcedAsyncStatusErrorMode::CopyFile =>
            {
                if payload.get("_async").and_then(Value::as_bool) == Some(true) {
                    Some(Ok(json!({ "jobid": state.job_id })))
                } else {
                    Some(Err(RcloneCliError::Io(io::Error::other(
                        "forced copy async test expected `_async: true` payload",
                    ))))
                }
            }
            RcloneRcMethod::OperationsDeleteFile
                if state.mode == ForcedAsyncStatusErrorMode::DeleteFile =>
            {
                if payload.get("_async").and_then(Value::as_bool) == Some(true) {
                    Some(Ok(json!({ "jobid": state.job_id })))
                } else {
                    Some(Err(RcloneCliError::Io(io::Error::other(
                        "forced delete async test expected `_async: true` payload",
                    ))))
                }
            }
            RcloneRcMethod::JobStatus => Some(Err(RcloneCliError::Io(io::Error::new(
                state.status_error_kind,
                "forced job/status transport error for tests",
            )))),
            RcloneRcMethod::JobStop => {
                state.job_stop_calls.fetch_add(1, Ordering::SeqCst);
                Some(Ok(json!({ "stopped": true })))
            }
            _ => None,
        }
    }
}

fn is_cancelled(cancel_token: Option<&AtomicBool>) -> bool {
    cancel_token
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::RcloneRcClient;
    use crate::commands::cloud::rclone_cli::RcloneCliError;
    use std::io;

    #[test]
    fn async_copy_job_status_error_returns_unknown_and_stops_job() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let client = RcloneRcClient::default()
            .with_enabled_override_for_tests(true)
            .with_forced_async_status_error_on_copy_for_tests(io::ErrorKind::ConnectionReset);

        let err = client
            .operations_copyfile(
                "work:",
                "src/file.txt",
                "work:",
                "dst/file.txt",
                Some(&cancel),
            )
            .expect_err("forced copy job/status error should fail");
        assert!(matches!(err, RcloneCliError::AsyncJobStateUnknown { .. }));
        assert_eq!(client.forced_job_stop_calls_for_tests(), 1);
    }

    #[test]
    fn async_delete_job_status_error_returns_unknown_and_stops_job() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let client = RcloneRcClient::default()
            .with_enabled_override_for_tests(true)
            .with_forced_async_status_error_on_delete_for_tests(io::ErrorKind::NotConnected);

        let err = client
            .operations_deletefile("work:", "dst/file.txt", Some(&cancel))
            .expect_err("forced delete job/status error should fail");
        assert!(matches!(err, RcloneCliError::AsyncJobStateUnknown { .. }));
        assert_eq!(client.forced_job_stop_calls_for_tests(), 1);
    }
}

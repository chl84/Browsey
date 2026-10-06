use super::{RcloneCliError, RcloneRcClient, RcloneRcMethod};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub(crate) struct RcCopyFileToLocalProgressSpec<'a> {
    pub src_fs: &'a str,
    pub src_remote: &'a str,
    pub dst_dir: &'a str,
    pub dst_remote: &'a str,
    pub group: &'a str,
    pub cancel_token: Option<&'a AtomicBool>,
}

pub(crate) struct RcCopyFileFromLocalProgressSpec<'a> {
    pub src_dir: &'a str,
    pub src_remote: &'a str,
    pub dst_fs: &'a str,
    pub dst_remote: &'a str,
    pub group: &'a str,
    pub cancel_token: Option<&'a AtomicBool>,
    pub refuse_replace: bool,
}

fn local_upload_payload(spec: &RcCopyFileFromLocalProgressSpec<'_>) -> Value {
    let mut payload = json!({
        "srcFs": {"type": "local", "_root": spec.src_dir},
        "srcRemote": spec.src_remote,
        "dstFs": spec.dst_fs,
        "dstRemote": spec.dst_remote,
    });
    if spec.refuse_replace {
        // Per-call config only: never change the daemon's global options.
        payload["_config"] = json!({"Immutable": true, "CheckSum": true, "IgnoreExisting": true});
    } else {
        payload["_config"] = json!({"IgnoreTimes": true});
    }
    payload
}

fn local_download_payload(
    src_fs: &str,
    src_remote: &str,
    dst_dir: &str,
    dst_remote: &str,
) -> Value {
    let mut destination = json!({"type":"local", "_root":dst_dir});
    if crate::fs_utils::is_mtp_destination(std::path::Path::new(dst_dir)) {
        // Backend options in fs objects are strings, scoped to this destination.
        // Never modify the daemon's global flags or replay a failed write.
        destination["no_set_modtime"] = json!("true");
    }
    json!({"srcFs": src_fs, "srcRemote": src_remote, "dstFs": destination, "dstRemote":dst_remote,
        "_config": {"IgnoreTimes": true}})
}

fn cloud_copy_payload(
    src_fs: &str,
    src_remote: &str,
    dst_fs: &str,
    dst_remote: &str,
    overwrite: bool,
) -> Value {
    let mut payload =
        json!({"srcFs": src_fs, "srcRemote": src_remote, "dstFs": dst_fs, "dstRemote": dst_remote});
    if overwrite {
        payload["_config"] = json!({"IgnoreTimes": true});
    }
    payload
}

impl RcloneRcClient {
    pub fn core_stats(&self, group: Option<&str>, short: bool) -> Result<Value, RcloneCliError> {
        let mut payload = json!({ "short": short });
        if let Some(group) = group {
            payload["group"] = Value::String(group.to_string());
        }
        self.run_method(RcloneRcMethod::CoreStats, payload)
    }

    pub fn core_stats_delete(&self, group: &str) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::CoreStatsDelete, json!({ "group": group }))
    }

    pub fn list_remotes(&self) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::ConfigListRemotes, json!({}))
    }

    pub fn config_dump(&self) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::ConfigDump, json!({}))
    }

    pub fn operations_list_with_options(
        &self,
        fs_spec: &str,
        remote_path: &str,
        timeout_override: Option<Duration>,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        self.run_method_with_options(
            RcloneRcMethod::OperationsList,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
            timeout_override,
            cancel_token,
        )
    }

    pub fn operations_stat_with_options(
        &self,
        fs_spec: &str,
        remote_path: &str,
        timeout_override: Option<Duration>,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        self.run_method_with_options(
            RcloneRcMethod::OperationsStat,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
            timeout_override,
            cancel_token,
        )
    }

    #[allow(dead_code)]
    pub fn operations_mkdir(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsMkdir,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    #[allow(dead_code)]
    pub fn operations_deletefile(
        &self,
        fs_spec: &str,
        remote_path: &str,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        let payload = json!({
            "fs": fs_spec,
            "remote": remote_path,
        });
        self.run_method_async_if_cancelable(
            RcloneRcMethod::OperationsDeleteFile,
            payload,
            cancel_token,
        )
    }

    #[allow(dead_code)]
    pub fn operations_purge(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsPurge,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    #[allow(dead_code)]
    pub fn operations_rmdir(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsRmdir,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    pub fn operations_copyfile(
        &self,
        src_fs: &str,
        src_remote: &str,
        dst_fs: &str,
        dst_remote: &str,
        overwrite: bool,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        let payload = cloud_copy_payload(src_fs, src_remote, dst_fs, dst_remote, overwrite);
        self.run_method_async_if_cancelable(
            RcloneRcMethod::OperationsCopyFile,
            payload,
            cancel_token,
        )
    }

    pub fn operations_copyfile_to_local_with_progress<F>(
        &self,
        spec: RcCopyFileToLocalProgressSpec<'_>,
        on_progress: F,
    ) -> Result<Value, RcloneCliError>
    where
        F: FnMut(Value),
    {
        let RcCopyFileToLocalProgressSpec {
            src_fs,
            src_remote,
            dst_dir,
            dst_remote,
            group,
            cancel_token,
        } = spec;
        let payload = local_download_payload(src_fs, src_remote, dst_dir, dst_remote);
        self.run_method_async_with_job_control_and_progress(
            RcloneRcMethod::OperationsCopyFile,
            payload,
            Some(group),
            cancel_token,
            on_progress,
        )
    }

    pub fn operations_copyfile_from_local_with_progress<F>(
        &self,
        spec: RcCopyFileFromLocalProgressSpec<'_>,
        on_progress: F,
    ) -> Result<Value, RcloneCliError>
    where
        F: FnMut(Value),
    {
        let payload = local_upload_payload(&spec);
        let RcCopyFileFromLocalProgressSpec {
            group,
            cancel_token,
            ..
        } = spec;
        self.run_method_async_with_job_control_and_progress(
            RcloneRcMethod::OperationsCopyFile,
            payload,
            Some(group),
            cancel_token,
            on_progress,
        )
    }

    pub fn operations_movefile(
        &self,
        src_fs: &str,
        src_remote: &str,
        dst_fs: &str,
        dst_remote: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsMoveFile,
            json!({
                "srcFs": src_fs,
                "srcRemote": src_remote,
                "dstFs": dst_fs,
                "dstRemote": dst_remote,
            }),
        )
    }

    pub(super) fn job_status(&self, job_id: u64) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::JobStatus, json!({ "jobid": job_id }))
    }

    pub(super) fn job_stop(&self, job_id: u64) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::JobStop, json!({ "jobid": job_id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_replace_upload_is_per_call_and_checks_content() {
        let mut spec = RcCopyFileFromLocalProgressSpec {
            src_dir: "/generated",
            src_remote: "source.txt",
            dst_fs: "work:",
            dst_remote: "target.txt",
            group: "test",
            cancel_token: None,
            refuse_replace: true,
        };
        let protected = local_upload_payload(&spec);
        assert_eq!(
            protected["_config"],
            json!({"Immutable": true, "CheckSum": true, "IgnoreExisting": true})
        );
        assert_eq!(protected["dstRemote"], "target.txt");
        spec.refuse_replace = false;
        assert_eq!(
            local_upload_payload(&spec)["_config"],
            json!({"IgnoreTimes": true})
        );
    }

    #[test]
    fn cloud_copy_force_is_per_call_and_only_for_explicit_overwrite() {
        assert!(cloud_copy_payload("work:", "src", "work:", "dst", false)
            .get("_config")
            .is_none());
        let overwrite = cloud_copy_payload("work:", "src", "work:", "dst", true);
        assert_eq!(overwrite["_config"], json!({"IgnoreTimes": true}));
        assert_eq!(overwrite["dstRemote"], "dst");
    }
}

#[cfg(test)]
#[test]
fn mtp_destination_rc_options_are_strings_scoped_to_the_destination_filesystem() {
    let mtp = local_download_payload(
        "work:",
        "file",
        "/run/user/123/gvfs/mtp:host=test/folder",
        "output",
    );
    assert_eq!(mtp["dstFs"]["no_set_modtime"], "true");
    assert_eq!(mtp["srcFs"], "work:");
    assert_eq!(mtp["_config"], json!({"IgnoreTimes": true}));
    let normal = local_download_payload("work:", "file", "/tmp/generated", "output");
    assert!(normal["dstFs"].get("no_set_modtime").is_none());
}

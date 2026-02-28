use super::{RcloneCliError, RcloneRcClient, RcloneRcMethod};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;

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

    pub fn operations_list(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsList,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    pub fn operations_stat(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsStat,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

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
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        let payload = json!({
            "srcFs": src_fs,
            "srcRemote": src_remote,
            "dstFs": dst_fs,
            "dstRemote": dst_remote,
        });
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
        let payload = json!({
            "srcFs": src_fs,
            "srcRemote": src_remote,
            "dstFs": {
                "type": "local",
                "_root": dst_dir,
            },
            "dstRemote": dst_remote,
        });
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
        let RcCopyFileFromLocalProgressSpec {
            src_dir,
            src_remote,
            dst_fs,
            dst_remote,
            group,
            cancel_token,
        } = spec;
        let payload = json!({
            "srcFs": {
                "type": "local",
                "_root": src_dir,
            },
            "srcRemote": src_remote,
            "dstFs": dst_fs,
            "dstRemote": dst_remote,
        });
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

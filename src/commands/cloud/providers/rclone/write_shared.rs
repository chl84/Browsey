use super::{
    CloudCommandError, CloudCommandErrorCode, CloudCommandResult, CloudPath, CloudProvider,
    RcloneCliError,
};
use std::sync::atomic::AtomicBool;

pub(super) fn ensure_destination_overwrite_policy(
    provider: &impl CloudProvider,
    src: &CloudPath,
    dst: &CloudPath,
    overwrite: bool,
) -> CloudCommandResult<()> {
    if overwrite || src == dst {
        return Ok(());
    }
    if provider.stat_path(dst)?.is_some() {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::DestinationExists,
            format!("Destination already exists: {dst}"),
        ));
    }
    Ok(())
}

pub(super) fn should_fallback_to_cli_after_rc_error(error: &RcloneCliError) -> bool {
    !matches!(error, RcloneCliError::AsyncJobStateUnknown { .. })
}

pub(super) fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
}

pub(super) fn cloud_write_cancelled_error() -> CloudCommandError {
    CloudCommandError::new(
        CloudCommandErrorCode::TaskFailed,
        "Cloud operation cancelled",
    )
}

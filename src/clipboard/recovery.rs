//! Recovery uses the same verified, cancellable stream as local paste, without
//! changing the clipboard or recording history against the retained backup.
use std::{path::Path, sync::atomic::AtomicBool};

use crate::errors::api_error::ApiResult;
use crate::fs_utils::{check_no_symlink_components, TreeSnapshot};

use super::{error::*, ops, progress::CopyProgress};

pub(crate) fn copy_recovery_backup(
    source: &Path,
    destination: &Path,
    app: Option<&tauri::AppHandle>,
    event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ApiResult<TreeSnapshot> {
    let result = (|| {
        check_no_symlink_components(source)?;
        crate::path_guard::ensure_no_symlink_components_existing_prefix(destination).map_err(
            |error| ClipboardError::invalid_input(format!("Unsafe recovery destination: {error}")),
        )?;
        let check = || {
            if ops::transfer_cancelled(cancel, app) {
                Err(std::io::ErrorKind::Interrupted.into())
            } else {
                Ok(())
            }
        };
        let snapshot = TreeSnapshot::capture_with_check(source, check).map_err(|error| {
            if error.kind() == std::io::ErrorKind::Interrupted {
                ClipboardError::cancelled()
            } else {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    "Inspect recovery backup",
                    error,
                )
            }
        })?;
        let total = super::clipboard_size::estimate_total_size(&[source.to_path_buf()], || {
            ops::transfer_cancelled(cancel, app)
        })?;
        let progress = event.map(|event| CopyProgress::new(app, event, total));
        let receipt = ops::copy_entry_with_receipt(
            source,
            destination,
            app,
            progress.as_ref(),
            cancel,
            true,
        )?;
        let verified = snapshot
            .verify_with_check(source, check)
            .map_err(|error| {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    "Recovery backup changed during copying",
                    error,
                )
            })
            .and_then(|()| {
                receipt
                    .verify_with_check(destination, check)
                    .map_err(ClipboardError::from)
            });
        if ops::transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        verified?;
        if let Some(progress) = progress {
            progress.finish();
        }
        Ok(snapshot)
    })();
    map_api_result(result.map_err(|error: ClipboardError| {
        error.with_context(format!(
            "Backup kept at {}. Inspect any incomplete output at {} before retrying",
            source.display(),
            destination.display(),
        ))
    }))
}

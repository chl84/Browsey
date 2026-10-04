//! Estimate file-content bytes, never directory inode sizes. Unknown totals stay
//! indeterminate; cancellation during preflight cannot start a mutation.
use std::path::PathBuf;

use super::error::{ClipboardError, ClipboardResult};

pub(super) fn estimate_total_size(
    entries: &[PathBuf],
    abort: impl Fn() -> bool + Sync,
) -> ClipboardResult<Option<u64>> {
    let mut total = 0u64;
    for root in entries {
        if abort() {
            return Err(ClipboardError::cancelled());
        }
        #[cfg(not(target_os = "windows"))]
        if root.to_string_lossy().contains("/gvfs/") {
            match super::gio_copy::estimate_size(root, &abort) {
                Ok(Some(size)) => {
                    let Some(sum) = total.checked_add(size) else {
                        return Ok(None);
                    };
                    total = sum;
                    continue;
                }
                Err(error) if error.code() == super::error::ClipboardErrorCode::Cancelled => {
                    return Err(error)
                }
                _ => return Ok(None),
            }
        }
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            if abort() {
                return Err(ClipboardError::cancelled());
            }
            let Ok(entry) = entry else { return Ok(None) };
            if entry.file_type().is_symlink() {
                return Ok(None);
            }
            if entry.file_type().is_file() {
                let Ok(meta) = entry.metadata() else {
                    return Ok(None);
                };
                let Some(sum) = total.checked_add(meta.len()) else {
                    return Ok(None);
                };
                total = sum;
            } else if !entry.file_type().is_dir() {
                return Ok(None);
            }
        }
    }
    if abort() {
        return Err(ClipboardError::cancelled());
    }
    Ok(Some(total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance_fixture::Fixture;
    use std::fs;

    #[test]
    fn counts_nested_file_contents_not_directory_inode_sizes() {
        let fixture = Fixture::new("copy-size-plan");
        fs::create_dir_all(fixture.0.join("empty/nested")).unwrap();
        fs::write(fixture.0.join("a"), [1; 13]).unwrap();
        fs::write(fixture.0.join("empty/b"), [2; 27]).unwrap();
        assert_eq!(
            estimate_total_size(std::slice::from_ref(&fixture.0), || false).unwrap(),
            Some(40)
        );
        assert_eq!(
            estimate_total_size(&[fixture.0.join("empty/nested")], || false).unwrap(),
            Some(0)
        );
    }

    #[test]
    fn cancelled_size_scan_never_becomes_an_unknown_total() {
        let fixture = Fixture::new("copy-size-cancel");
        assert_eq!(
            estimate_total_size(std::slice::from_ref(&fixture.0), || true)
                .unwrap_err()
                .code(),
            super::super::error::ClipboardErrorCode::Cancelled
        );
    }

    #[test]
    fn unavailable_metadata_has_an_unknown_total_not_fake_item_bytes() {
        let fixture = Fixture::new("copy-size-unknown");
        assert_eq!(
            estimate_total_size(&[fixture.0.join("missing")], || false).unwrap(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn size_scan_does_not_follow_symlinks() {
        let fixture = Fixture::new("copy-size-symlink");
        fs::write(fixture.0.join("source"), [1; 17]).unwrap();
        std::os::unix::fs::symlink(fixture.0.join("source"), fixture.0.join("link")).unwrap();
        assert_eq!(
            estimate_total_size(&[fixture.0.join("link")], || false).unwrap(),
            None
        );
    }
}

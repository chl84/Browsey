use crate::commands::decompress::error::DecompressResult;
use std::{fs::File, path::Path};

/// Restore ordinary permission bits only, never setuid/setgid/sticky bits.
pub(crate) fn restore_file_mode(file: &File, mode: Option<u32>) -> DecompressResult<()> {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(mode & 0o777))
            .map_err(|e| format!("Failed to restore extracted file permissions: {e}"))?;
    }
    #[cfg(not(unix))]
    let _ = (file, mode);
    Ok(())
}

pub(super) fn restore_directory_mode(path: &Path, mode: u32) -> DecompressResult<()> {
    #[cfg(target_os = "linux")]
    crate::undo::set_unix_mode_nofollow(path, mode & 0o777)
        .map_err(|e| format!("Failed to restore extracted directory permissions: {e}"))?;
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use std::os::unix::fs::PermissionsExt;
        crate::fs_utils::check_no_symlink_components(path)
            .map_err(|e| format!("Unsafe extracted directory: {e}"))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o777))
            .map_err(|e| format!("Failed to restore extracted directory permissions: {e}"))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

pub(crate) fn sevenz_mode(has_attributes: bool, attributes: u32) -> Option<u32> {
    // 7z's Unix extension marker, with POSIX mode in the upper 16 bits.
    (has_attributes && attributes & 0x8000 != 0).then_some(attributes >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_attributes_are_not_unix_permissions() {
        assert_eq!(sevenz_mode(true, 0x20), None);
        assert_eq!(sevenz_mode(false, 0x81ed8000), None);
        assert_eq!(sevenz_mode(true, 0x81ed8000), Some(0o100755));
    }
}

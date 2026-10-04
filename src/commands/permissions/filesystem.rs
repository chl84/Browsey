use std::path::Path;

use super::{error::PermissionsResult, PermissionRestriction};

#[derive(Debug, Clone, Copy)]
pub(super) struct FatPermissions {
    pub write_mask: u32,
    pub can_toggle: bool,
}

impl FatPermissions {
    pub fn restriction(self) -> PermissionRestriction {
        if self.can_toggle {
            PermissionRestriction::WriteProtection
        } else {
            PermissionRestriction::MountManaged
        }
    }

    pub fn updated_mode(
        self,
        mode: u32,
        read_only: Option<bool>,
        executable: Option<bool>,
        updates: [Option<&super::AccessUpdate>; 3],
    ) -> PermissionsResult<u32> {
        let [owner, group, other] = updates;
        let has_changes = |update: &super::AccessUpdate| {
            update.read.is_some() || update.write.is_some() || update.exec.is_some()
        };
        if !self.can_toggle
            || executable.is_some()
            || owner.is_some_and(|update| update.read.is_some() || update.exec.is_some())
            || group.is_some_and(has_changes)
            || other.is_some_and(has_changes)
        {
            return Err(super::error::PermissionsError::invalid_input(
                "This filesystem only supports file write protection; other permissions are controlled by mount options.",
            ));
        }
        let write = owner
            .and_then(|update| update.write)
            .or(read_only.map(|value| !value));
        Ok(match write {
            Some(true) => (mode & !0o222) | self.write_mask,
            Some(false) => mode & !0o222,
            None => mode,
        })
    }
}

#[cfg(target_os = "linux")]
pub(super) fn fat_permissions(
    path: &Path,
    is_dir: bool,
) -> PermissionsResult<Option<FatPermissions>> {
    use super::error::{PermissionsError, PermissionsErrorCode};
    use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt};

    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| PermissionsError::invalid_input("Invalid filesystem path"))?;
    let mut stats = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: path is NUL-terminated; statfs writes the complete result on success.
    if unsafe { libc::statfs(c_path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(PermissionsError::from_io_error(
            PermissionsErrorCode::MetadataReadFailed,
            "Failed to read filesystem capabilities",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: statfs succeeded above.
    let magic = unsafe { stats.assume_init() }.f_type;
    if magic != 0x2011_bab0 && magic != 0x4d44 {
        return Ok(None);
    }
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").map_err(|error| {
        PermissionsError::from_io_error(
            PermissionsErrorCode::MetadataReadFailed,
            "Failed to read filesystem mount options",
            error,
        )
    })?;
    // Missing/unknown masks are deliberately read-only in the UI, never guessed.
    Ok(Some(fat_mount_permissions(&mounts, path, is_dir)))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn fat_permissions(
    _path: &Path,
    _is_dir: bool,
) -> PermissionsResult<Option<FatPermissions>> {
    Ok(None)
}

#[cfg(target_os = "linux")]
fn decode_mount_path(raw: &str) -> std::path::PathBuf {
    // mountinfo escapes whitespace and backslashes with octal sequences.
    raw.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
        .into()
}

#[cfg(target_os = "linux")]
fn fat_mount_permissions(mounts: &str, path: &Path, is_dir: bool) -> FatPermissions {
    let mut selected = None;
    let mut longest = 0;
    for line in mounts.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let fields: Vec<_> = left.split_whitespace().collect();
        let super_fields: Vec<_> = right.split_whitespace().collect();
        if fields.len() < 6 || super_fields.len() < 3 {
            continue;
        }
        let mount = decode_mount_path(fields[4]);
        if !path.starts_with(&mount) || mount.as_os_str().len() < longest {
            continue;
        }
        longest = mount.as_os_str().len();
        let options: Vec<_> = fields[5]
            .split(',')
            .chain(super_fields[2].split(','))
            .collect();
        let mask = options
            .iter()
            .find_map(|option| option.strip_prefix("fmask="))
            .or_else(|| {
                options
                    .iter()
                    .find_map(|option| option.strip_prefix("umask="))
            })
            .and_then(|mask| u32::from_str_radix(mask, 8).ok());
        let write_mask = mask.map(|mask| 0o222 & !mask).unwrap_or(0);
        selected = Some(FatPermissions {
            write_mask,
            // Directories on exFAT do not implement the file read-only attribute.
            // Be conservative for FAT directories too, regardless of rodir.
            can_toggle: !is_dir && write_mask & 0o200 != 0 && !options.contains(&"ro"),
        });
    }
    selected.unwrap_or(FatPermissions {
        write_mask: 0,
        can_toggle: false,
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn fat_mount_masks_and_directory_restrictions() {
        for fs in ["exfat", "vfat", "msdos"] {
            let mounts =
                format!("1 0 8:1 / /media/USB rw - {fs} /dev/sda1 rw,fmask=0022,dmask=0022");
            let file = fat_mount_permissions(&mounts, Path::new("/media/USB/file.pdf"), false);
            assert_eq!(file.write_mask, 0o200);
            assert_eq!(file.restriction(), PermissionRestriction::WriteProtection);
            assert_eq!(
                fat_mount_permissions(&mounts, Path::new("/media/USB/dir"), true).restriction(),
                PermissionRestriction::MountManaged
            );
        }
    }

    #[test]
    fn mount_masks_read_only_and_unknown_options_are_not_editable() {
        for options in ["ro,fmask=0022", "rw,fmask=0222", "rw", "rw,fmask=invalid"] {
            let mounts = format!("1 0 8:1 / /media/USB rw - exfat /dev/sda1 {options}");
            assert!(
                !fat_mount_permissions(&mounts, Path::new("/media/USB/file"), false).can_toggle
            );
        }
        let mounts = "1 0 8:1 / /media/USB rw - exfat /dev/sda1 rw,umask=0002";
        assert_eq!(
            fat_mount_permissions(mounts, Path::new("/media/USB/file"), false).write_mask,
            0o220
        );
    }

    #[test]
    fn escaped_paths_nested_mounts_and_component_boundaries() {
        let mounts = "1 0 8:1 / /media/My\\040USB rw - exfat /dev/sda1 rw,fmask=0022\n2 1 8:2 / /media/My\\040USB/nested ro - vfat /dev/sdb1 ro,fmask=0022";
        assert!(fat_mount_permissions(mounts, Path::new("/media/My USB/file"), false).can_toggle);
        assert!(
            !fat_mount_permissions(mounts, Path::new("/media/My USB/nested/file"), false)
                .can_toggle
        );
        assert!(
            !fat_mount_permissions(mounts, Path::new("/media/My USB-other/file"), false).can_toggle
        );
    }

    #[test]
    fn ordinary_local_filesystem_has_no_fat_restriction() {
        assert!(fat_permissions(Path::new(env!("CARGO_MANIFEST_DIR")), true)
            .unwrap()
            .is_none());
    }

    #[test]
    fn write_protection_toggles_all_unmasked_write_bits_without_granting_new_access() {
        let fat = FatPermissions {
            write_mask: 0o220,
            can_toggle: true,
        };
        let owner = super::super::AccessUpdate {
            read: None,
            write: Some(false),
            exec: None,
        };
        assert_eq!(
            fat.updated_mode(0o775, None, None, [Some(&owner), None, None])
                .unwrap(),
            0o555
        );
        let owner = super::super::AccessUpdate {
            write: Some(true),
            ..owner
        };
        assert_eq!(
            fat.updated_mode(0o555, None, None, [Some(&owner), None, None])
                .unwrap(),
            0o775
        );
        assert_eq!(
            fat.updated_mode(0o775, Some(true), None, [None, None, None])
                .unwrap(),
            0o555
        );
    }

    #[test]
    fn unsupported_fat_updates_are_rejected_before_any_change() {
        let fat = FatPermissions {
            write_mask: 0o200,
            can_toggle: true,
        };
        let read = super::super::AccessUpdate {
            read: Some(false),
            write: None,
            exec: None,
        };
        let write = super::super::AccessUpdate {
            read: None,
            write: Some(false),
            exec: None,
        };
        assert!(fat
            .updated_mode(0o755, None, None, [Some(&read), None, None])
            .is_err());
        assert!(fat
            .updated_mode(0o755, None, Some(false), [None, None, None])
            .is_err());
        assert!(fat
            .updated_mode(0o755, None, None, [None, Some(&write), None])
            .is_err());
        assert!(FatPermissions {
            can_toggle: false,
            ..fat
        }
        .updated_mode(0o755, Some(true), None, [None, None, None])
        .is_err());
    }
}

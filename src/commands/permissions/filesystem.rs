use std::path::Path;

use super::{
    error::{PermissionsError, PermissionsErrorCode, PermissionsResult},
    PermissionRestriction,
};

#[derive(Debug, Default)]
pub(super) struct FilesystemCapabilities {
    pub restriction: Option<PermissionRestriction>,
    pub fat: Option<FatPermissions>,
    pub network: bool,
}

impl FilesystemCapabilities {
    pub fn ensure_editable(&self, ownership: bool) -> PermissionsResult<()> {
        match self.restriction {
            None => Ok(()),
            Some(PermissionRestriction::WriteProtection) if !ownership => Ok(()),
            Some(PermissionRestriction::ReadOnly) => Err(PermissionsError::new(
                PermissionsErrorCode::ReadOnlyFilesystem,
                "This filesystem is mounted read-only.",
            )),
            Some(PermissionRestriction::NetworkManaged) => Err(PermissionsError::new(
                PermissionsErrorCode::UnsupportedFilesystem,
                "Permissions are managed by the server.",
            )),
            _ => Err(PermissionsError::new(
                PermissionsErrorCode::UnsupportedFilesystem,
                "Permissions are controlled by mount options.",
            )),
        }
    }
}

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
pub(super) fn filesystem_capabilities(
    path: &Path,
    is_dir: bool,
) -> PermissionsResult<FilesystemCapabilities> {
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").map_err(|error| {
        PermissionsError::from_io_error(
            PermissionsErrorCode::MetadataReadFailed,
            "Failed to read filesystem mount options",
            error,
        )
    })?;
    let mount = mount_for_path(&mounts, path);
    Ok(classify_mount(mount.as_ref(), is_dir))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn filesystem_capabilities(
    _path: &Path,
    _is_dir: bool,
) -> PermissionsResult<FilesystemCapabilities> {
    Ok(FilesystemCapabilities::default())
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
struct MountInfo<'a> {
    root: std::path::PathBuf,
    fs: &'a str,
    options: Vec<&'a str>,
}

#[cfg(target_os = "linux")]
fn mount_for_path<'a>(mounts: &'a str, path: &Path) -> Option<MountInfo<'a>> {
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
        selected = Some(MountInfo {
            root: mount,
            fs: super_fields[0],
            options,
        });
    }
    selected
}

#[cfg(target_os = "linux")]
pub(super) fn network_mount_root(path: &Path) -> PermissionsResult<Option<std::path::PathBuf>> {
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").map_err(|error| {
        PermissionsError::from_io_error(
            PermissionsErrorCode::MetadataReadFailed,
            "Failed to read filesystem mount options",
            error,
        )
    })?;
    let mount = mount_for_path(&mounts, path);
    Ok(mount
        .filter(|mount| classify_mount(Some(mount), false).network)
        .map(|mount| mount.root))
}

#[cfg(target_os = "linux")]
fn classify_mount(mount: Option<&MountInfo<'_>>, is_dir: bool) -> FilesystemCapabilities {
    let Some(mount) = mount else {
        return FilesystemCapabilities {
            restriction: Some(PermissionRestriction::MountManaged),
            ..Default::default()
        };
    };
    let fs = mount.fs;
    let options = &mount.options;
    let has = |flag| options.contains(&flag);
    let network = matches!(
        fs,
        "nfs"
            | "nfs4"
            | "cifs"
            | "smb3"
            | "smbfs"
            | "9p"
            | "ceph"
            | "afs"
            | "coda"
            | "davfs"
            | "davfs2"
            | "fuse.davfs"
            | "fuse.sshfs"
            | "sshfs"
            | "fuse.gvfsd-fuse"
            | "fuse.rclone"
            | "fuse.curlftpfs"
    );
    // mountinfo reports per-mount and superblock flags, including read-only
    // bind mounts. Read-only takes precedence over every other capability.
    if has("ro") {
        return FilesystemCapabilities {
            restriction: Some(PermissionRestriction::ReadOnly),
            network,
            fat: None,
        };
    }
    if matches!(fs, "exfat" | "vfat" | "msdos" | "fuse.exfat") {
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
        let fat = FatPermissions {
            write_mask,
            // Directories on exFAT do not implement the file read-only attribute.
            // Be conservative for FAT directories too, regardless of rodir.
            can_toggle: !is_dir && write_mask & 0o200 != 0 && !options.contains(&"ro"),
        };
        return FilesystemCapabilities {
            restriction: Some(fat.restriction()),
            fat: Some(fat),
            network: false,
        };
    }
    let restriction = match fs {
        "ntfs3" => {
            if has("noacsrules") || has("inherit") {
                Some(PermissionRestriction::MountManaged)
            } else {
                None
            }
        }
        // fuseblk does not reliably expose which driver is running or whether
        // its NTFS user mapping is active. Do not infer support from mode bits.
        "fuseblk" => Some(PermissionRestriction::MountManaged),
        "ntfs" => Some(PermissionRestriction::MountManaged),
        "ntfs-3g" | "fuse.ntfs-3g" | "fuse.lowntfs-3g" => {
            if !has("inherit") && (has("permissions") || has("acl")) {
                None
            } else {
                Some(PermissionRestriction::MountManaged)
            }
        }
        "cifs" | "smb3" => {
            if !has("dynperm") && (has("unix") || has("posix") || has("cifsacl")) {
                None
            } else {
                Some(PermissionRestriction::NetworkManaged)
            }
        }
        // NFS and SSHFS support chmod/chown, but server authorization still
        // decides each request. Verification below must not trust syscall success.
        "nfs" | "nfs4" | "fuse.sshfs" | "sshfs" => None,
        _ if network => Some(PermissionRestriction::NetworkManaged),
        _ => None,
    };
    FilesystemCapabilities {
        restriction,
        fat: None,
        network,
    }
}

#[cfg(all(test, target_os = "linux"))]
fn fat_mount_permissions(mounts: &str, path: &Path, is_dir: bool) -> FatPermissions {
    classify_mount(mount_for_path(mounts, path).as_ref(), is_dir)
        .fat
        .unwrap_or(FatPermissions {
            write_mask: 0,
            can_toggle: false,
        })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn caps(fs: &str, options: &str) -> FilesystemCapabilities {
        let text = format!("1 0 8:1 / /volume rw - {fs} device {options}");
        classify_mount(
            mount_for_path(&text, Path::new("/volume/file")).as_ref(),
            false,
        )
    }

    #[test]
    fn read_only_overrides_local_ntfs_and_network_capabilities() {
        for fs in ["ext4", "btrfs", "xfs", "ntfs3", "exfat", "nfs4", "cifs"] {
            let result = caps(fs, "ro,unix,fmask=0022");
            assert_eq!(result.restriction, Some(PermissionRestriction::ReadOnly));
            assert_eq!(
                result.ensure_editable(false).unwrap_err().code(),
                "read_only_filesystem"
            );
            assert_eq!(
                result.ensure_editable(true).unwrap_err().code(),
                "read_only_filesystem"
            );
        }
        let text = "1 0 8:1 / /volume ro - ext4 device rw";
        let mount = mount_for_path(text, Path::new("/volume/file"));
        assert_eq!(
            classify_mount(mount.as_ref(), false).restriction,
            Some(PermissionRestriction::ReadOnly)
        );
        let text = "1 0 8:1 / /volume rw - ext4 device ro";
        let mount = mount_for_path(text, Path::new("/volume/file"));
        assert_eq!(
            classify_mount(mount.as_ref(), false).restriction,
            Some(PermissionRestriction::ReadOnly)
        );
    }

    #[test]
    fn ntfs_driver_and_permission_options_are_not_interchangeable() {
        assert!(caps("ntfs3", "rw,uid=1000,fmask=0022")
            .restriction
            .is_none());
        for (fs, options) in [
            ("ntfs3", "rw,noacsrules"),
            ("ntfs3", "rw,inherit"),
            ("ntfs", "rw,permissions"),
            ("fuseblk", "rw,user_id=0,group_id=0"),
            ("ntfs-3g", "rw,silent,uid=1000"),
            ("fuse.ntfs-3g", "rw,permissions,inherit"),
            ("fuse.ntfs-3g", "rw,usermapping=/mapping"),
        ] {
            assert_eq!(
                caps(fs, options).restriction,
                Some(PermissionRestriction::MountManaged)
            );
        }
        for fs in ["ntfs-3g", "fuse.ntfs-3g", "fuse.lowntfs-3g"] {
            assert!(caps(fs, "rw,permissions").restriction.is_none());
            assert!(caps(fs, "rw,acl").restriction.is_none());
        }
    }

    #[test]
    fn remote_permissions_require_known_persistent_support() {
        for fs in ["cifs", "smb3"] {
            for options in [
                "rw,nounix",
                "rw,nounix,dynperm",
                "rw,unix,dynperm",
                "rw,posixpaths",
            ] {
                let result = caps(fs, options);
                assert!(result.network);
                assert_eq!(
                    result.restriction,
                    Some(PermissionRestriction::NetworkManaged)
                );
                assert_eq!(
                    result.ensure_editable(false).unwrap_err().code(),
                    "unsupported_filesystem"
                );
            }
            for options in ["rw,unix", "rw,posix", "rw,cifsacl"] {
                let result = caps(fs, options);
                assert!(result.network && result.restriction.is_none());
            }
        }
        for fs in ["nfs", "nfs4", "fuse.sshfs"] {
            let result = caps(fs, "rw");
            assert!(result.network && result.restriction.is_none());
        }
        for fs in ["fuse.gvfsd-fuse", "fuse.rclone", "davfs2", "fuse.curlftpfs"] {
            assert_eq!(
                caps(fs, "rw").restriction,
                Some(PermissionRestriction::NetworkManaged)
            );
        }
    }

    #[test]
    fn nested_mount_wins_without_matching_a_similarly_named_sibling() {
        let text = "1 0 8:1 / / rw - btrfs disk rw\n2 1 0:42 / /volume rw - cifs share rw,nounix\n3 2 8:2 / /volume/local rw - ext4 disk rw";
        let classify = |path| classify_mount(mount_for_path(text, Path::new(path)).as_ref(), false);
        assert_eq!(
            classify("/volume/file").restriction,
            Some(PermissionRestriction::NetworkManaged)
        );
        assert!(classify("/volume/local/file").restriction.is_none());
        assert!(classify("/volume-other/file").restriction.is_none());
        assert_eq!(
            classify_mount(None, false).restriction,
            Some(PermissionRestriction::MountManaged)
        );
    }

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
        let caps = filesystem_capabilities(Path::new(env!("CARGO_MANIFEST_DIR")), true).unwrap();
        assert!(caps.fat.is_none());
        assert!(caps.restriction.is_none());
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

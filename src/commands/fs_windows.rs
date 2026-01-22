use super::MountInfo;
use std::ffi::OsString;
use std::iter;
use std::os::windows::ffi::OsStringExt;
use std::process::Command;
use windows_sys::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
};
use windows_sys::Win32::System::WindowsProgramming::{
    DRIVE_CDROM, DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
};

pub fn list_windows_mounts() -> Vec<MountInfo> {
    // First call to get required buffer length (in WCHARs, including trailing null).
    let len = unsafe { GetLogicalDriveStringsW(0, std::ptr::null_mut()) };
    if len == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let got = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) };
    if got == 0 {
        return Vec::new();
    }

    let mut mounts = Vec::new();
    let mut start = 0;
    for i in 0..buf.len() {
        if buf[i] != 0 {
            continue;
        }
        if i == start {
            start += 1;
            continue;
        }
        let drive = OsString::from_wide(&buf[start..i])
            .to_string_lossy()
            .into_owned();
        start = i + 1;
        if drive.is_empty() {
            continue;
        }

        let drive_w: Vec<u16> = drive.encode_utf16().chain(iter::once(0)).collect();
        let drive_type = unsafe { GetDriveTypeW(drive_w.as_ptr()) };
        match drive_type {
            DRIVE_FIXED | DRIVE_REMOVABLE | DRIVE_REMOTE | DRIVE_RAMDISK | DRIVE_CDROM => {}
            _ => continue,
        }

        // Volume and filesystem names
        let mut vol_name = vec![0u16; 260];
        let mut fs_name = vec![0u16; 260];
        let vol_ok = unsafe {
            GetVolumeInformationW(
                drive_w.as_ptr(),
                vol_name.as_mut_ptr(),
                vol_name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs_name.as_mut_ptr(),
                fs_name.len() as u32,
            )
        };
        let label = if vol_ok != 0 {
            utf16_to_string(&vol_name).unwrap_or_else(|| drive.clone())
        } else {
            drive.clone()
        };
        let fs = if vol_ok != 0 {
            utf16_to_string(&fs_name).unwrap_or_default()
        } else {
            String::new()
        };

        mounts.push(MountInfo {
            label,
            path: drive,
            fs,
            removable: matches!(drive_type, DRIVE_REMOVABLE | DRIVE_CDROM),
        });
    }

    mounts
}

fn utf16_to_string(buf: &[u16]) -> Option<String> {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..end]))
}

pub fn eject_drive(path: &str) -> Result<(), String> {
    // Expect something like "D:\"; normaliser til "D:" for Shell.Application.
    let drive = path.trim_end_matches(['\\', '/']);
    let mut chars = drive.chars();
    let letter = chars.next().ok_or_else(|| "Invalid drive path".to_string())?;
    if !letter.is_ascii_alphabetic() {
        return Err("Invalid drive path".into());
    }
    let target = format!("{}:", letter.to_ascii_uppercase());
    // Use COM Shell.Application verb "Eject" via PowerShell to avoid extra deps.
    let ps = format!(
        "$d='{target}'; $app=New-Object -ComObject Shell.Application; $item=$app.NameSpace(17).ParseName($d); if($item){{ $item.InvokeVerb('Eject'); exit 0 }} else {{ exit 1 }}"
    );
    let status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(ps)
        .status()
        .map_err(|e| format!("Failed to spawn PowerShell: {e}"))?;
    if status.success() {
        return Ok(());
    }

    // Fallback: try to dismount using mountvol /p
    let mv_status = Command::new("mountvol")
        .arg(format!("{target}\\"))
        .arg("/p")
        .status()
        .map_err(|e| format!("Failed to run mountvol: {e}"))?;
    if mv_status.success() {
        Ok(())
    } else {
        Err(format!("Failed to eject drive {target}"))
    }
}

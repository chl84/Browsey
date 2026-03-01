use super::{
    error::{FsError, FsErrorCode, FsResult},
    MountInfo,
};
use std::ffi::OsString;
use std::fs;
use std::fs::OpenOptions;
use std::iter;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::Command;
use std::ptr;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Request_Device_EjectW, PNP_VetoTypeUnknown, SetupDiDestroyDeviceInfoList,
    SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW, CONFIGRET,
    CR_REMOVE_VETOED, CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, PNP_VETO_TYPE,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};
use windows_sys::Win32::System::Ioctl::{
    FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_EJECT_MEDIA,
    IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_MEDIA_REMOVAL, PREVENT_MEDIA_REMOVAL,
    STORAGE_DEVICE_NUMBER,
};
use windows_sys::Win32::System::WindowsProgramming::{
    DRIVE_CDROM, DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

pub fn list_windows_mounts() -> FsResult<Vec<MountInfo>> {
    // First call to get required buffer length (in WCHARs, including trailing null).
    let len = unsafe { GetLogicalDriveStringsW(0, std::ptr::null_mut()) };
    if len == 0 {
        return Err(FsError::new(
            FsErrorCode::TaskFailed,
            "GetLogicalDriveStringsW failed to report buffer length",
        ));
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let got = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) };
    if got == 0 {
        return Err(FsError::new(
            FsErrorCode::TaskFailed,
            "GetLogicalDriveStringsW failed to enumerate mounts",
        ));
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

        // After an eject Windows may leave the drive letter but report NOT_READY; skip such removable volumes.
        if drive_type == DRIVE_REMOVABLE {
            if let Err(err) = fs::metadata(&drive) {
                match err.raw_os_error() {
                    Some(21)   // ERROR_NOT_READY
                    | Some(1110) // ERROR_MEDIA_CHANGED
                    | Some(1167) // ERROR_DEVICE_NOT_CONNECTED
                    | Some(2) => {
                        // treat as gone
                        continue;
                    }
                    _ => {}
                }
            }
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

    Ok(mounts)
}

fn utf16_to_string(buf: &[u16]) -> Option<String> {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..end]))
}

fn device_number_for_drive(letter: char) -> FsResult<u32> {
    let path = format!(r"\\.\{}:", letter);
    unsafe {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(Path::new(&path))
            .map_err(|e| {
                FsError::new(
                    FsErrorCode::OpenFailed,
                    format!("Failed to open {path}: {e}"),
                )
            })?;
        let handle = file.as_raw_handle();

        let mut dev_num: STORAGE_DEVICE_NUMBER = zeroed();
        let mut bytes = 0;
        let ok = DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            ptr::null_mut(),
            0,
            &mut dev_num as *mut _ as *mut _,
            size_of::<STORAGE_DEVICE_NUMBER>() as u32,
            &mut bytes,
            ptr::null_mut(),
        );
        if ok == 0 {
            let err = GetLastError();
            return Err(FsError::new(
                FsErrorCode::TaskFailed,
                format!("IOCTL_STORAGE_GET_DEVICE_NUMBER failed for {path}: Win32 error {err}"),
            ));
        }
        Ok(dev_num.DeviceNumber)
    }
}

fn lock_and_eject_media(letter: char) -> FsResult<()> {
    let path = format!(r"\\.\{}:", letter);
    unsafe {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(Path::new(&path))
            .map_err(|e| {
                FsError::new(
                    FsErrorCode::OpenFailed,
                    format!("Failed to open {path}: {e}"),
                )
            })?;
        let handle = file.as_raw_handle();

        let mut bytes = 0u32;
        let ok_lock = DeviceIoControl(
            handle,
            FSCTL_LOCK_VOLUME,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
            &mut bytes,
            ptr::null_mut(),
        );
        if ok_lock == 0 {
            let err = GetLastError();
            return Err(FsError::new(
                FsErrorCode::TaskFailed,
                format!("FSCTL_LOCK_VOLUME failed: Win32 error {err}"),
            ));
        }

        let ok_dismount = DeviceIoControl(
            handle,
            FSCTL_DISMOUNT_VOLUME,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
            &mut bytes,
            ptr::null_mut(),
        );
        if ok_dismount == 0 {
            let err = GetLastError();
            return Err(FsError::new(
                FsErrorCode::TaskFailed,
                format!("FSCTL_DISMOUNT_VOLUME failed: Win32 error {err}"),
            ));
        }

        let mut pmr = PREVENT_MEDIA_REMOVAL {
            PreventMediaRemoval: false,
        };
        let ok_allow = DeviceIoControl(
            handle,
            IOCTL_STORAGE_MEDIA_REMOVAL,
            &mut pmr as *mut _ as *mut _,
            size_of::<PREVENT_MEDIA_REMOVAL>() as u32,
            ptr::null_mut(),
            0,
            &mut bytes,
            ptr::null_mut(),
        );
        if ok_allow == 0 {
            let err = GetLastError();
            return Err(FsError::new(
                FsErrorCode::TaskFailed,
                format!("IOCTL_STORAGE_MEDIA_REMOVAL failed: Win32 error {err}"),
            ));
        }

        let ok_eject = DeviceIoControl(
            handle,
            IOCTL_STORAGE_EJECT_MEDIA,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
            &mut bytes,
            ptr::null_mut(),
        );
        if ok_eject == 0 {
            let err = GetLastError();
            return Err(FsError::new(
                FsErrorCode::TaskFailed,
                format!("IOCTL_STORAGE_EJECT_MEDIA failed: Win32 error {err}"),
            ));
        }
        Ok(())
    }
}

struct DevInfoList(HDEVINFO);
impl Drop for DevInfoList {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

fn eject_by_device_number(device_number: u32) -> FsResult<()> {
    unsafe {
        let hdev = SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_DISK,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if hdev == INVALID_HANDLE_VALUE as isize {
            return Err(FsError::new(
                FsErrorCode::TaskFailed,
                "SetupDiGetClassDevsW failed",
            ));
        }
        let _guard = DevInfoList(hdev);
        let mut index = 0;
        loop {
            let mut iface_data: SP_DEVICE_INTERFACE_DATA = zeroed();
            iface_data.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(
                hdev,
                ptr::null_mut(),
                &GUID_DEVINTERFACE_DISK,
                index,
                &mut iface_data,
            ) == 0
            {
                let err = GetLastError();
                if err == ERROR_NO_MORE_ITEMS {
                    break;
                }
                return Err(FsError::new(
                    FsErrorCode::TaskFailed,
                    format!("SetupDiEnumDeviceInterfaces failed: Win32 error {err}"),
                ));
            }
            index += 1;

            // Find required buffer size
            let mut required = 0;
            SetupDiGetDeviceInterfaceDetailW(
                hdev,
                &mut iface_data,
                ptr::null_mut(),
                0,
                &mut required,
                ptr::null_mut(),
            );
            if required == 0 {
                continue;
            }
            let mut detail_buf = vec![0u8; required as usize];
            let detail_ptr = detail_buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail_ptr).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            let mut dev_info: SP_DEVINFO_DATA = zeroed();
            dev_info.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;

            if SetupDiGetDeviceInterfaceDetailW(
                hdev,
                &mut iface_data,
                detail_ptr,
                required,
                ptr::null_mut(),
                &mut dev_info,
            ) == 0
            {
                continue;
            }

            // DevicePath is a null-terminated UTF-16 string
            let dev_path_ptr = (*detail_ptr).DevicePath.as_ptr();
            let mut len = 0usize;
            while *dev_path_ptr.add(len) != 0 {
                len += 1;
            }
            let dev_path = String::from_utf16_lossy(std::slice::from_raw_parts(dev_path_ptr, len));
            let file = match OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .open(Path::new(&dev_path))
            {
                Ok(f) => f,
                Err(_) => continue,
            };
            let handle = file.as_raw_handle();

            let mut dev_num: STORAGE_DEVICE_NUMBER = zeroed();
            let mut bytes = 0;
            let ok = DeviceIoControl(
                handle,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                ptr::null_mut(),
                0,
                &mut dev_num as *mut _ as *mut _,
                size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                &mut bytes,
                ptr::null_mut(),
            );
            if ok == 0 || dev_num.DeviceNumber != device_number {
                continue;
            }

            let mut veto_type: PNP_VETO_TYPE = PNP_VetoTypeUnknown;
            let mut veto_name = [0u16; 256];
            let veto_len = veto_name.len() as u32;
            let cr: CONFIGRET = CM_Request_Device_EjectW(
                dev_info.DevInst,
                &mut veto_type,
                veto_name.as_mut_ptr(),
                veto_len,
                0,
            );
            return match cr {
                CR_SUCCESS => Ok(()),
                CR_REMOVE_VETOED => {
                    let reason = utf16_to_string(&veto_name).unwrap_or_else(|| "unknown".into());
                    Err(FsError::new(
                        FsErrorCode::TaskFailed,
                        format!("Eject vetoed: {reason}"),
                    ))
                }
                other => Err(FsError::new(
                    FsErrorCode::TaskFailed,
                    format!("CM_Request_Device_EjectW failed: code {other}"),
                )),
            };
        }
    }
    Err(FsError::new(
        FsErrorCode::NotFound,
        "No matching device found to eject",
    ))
}

pub fn eject_drive(path: &str) -> FsResult<()> {
    // Expect something like "D:\"; normalize to "D:".
    let drive = path.trim_end_matches(['\\', '/']);
    let mut chars = drive.chars();
    let letter = chars
        .next()
        .ok_or_else(|| FsError::new(FsErrorCode::InvalidPath, "Invalid drive path"))?;
    if !letter.is_ascii_alphabetic() {
        return Err(FsError::new(FsErrorCode::InvalidPath, "Invalid drive path"));
    }
    let target = format!("{}:", letter.to_ascii_uppercase());

    // Primary path: ask Configuration Manager to eject the device (same mechanism as Explorer).
    match device_number_for_drive(letter.to_ascii_uppercase()) {
        Ok(dev_num) => match eject_by_device_number(dev_num) {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::warn!("Device eject via CfgMgr32 failed for {}: {}", target, e);
            }
        },
        Err(e) => {
            tracing::warn!("Device number lookup failed for {}: {}", target, e);
        }
    }

    // Fallback: try locking, dismounting, and ejecting the media on the volume handle.
    if let Err(e) = lock_and_eject_media(letter.to_ascii_uppercase()) {
        tracing::warn!("Volume lock/eject failed for {}: {}", target, e);
    } else {
        return Ok(());
    }

    // Use COM Shell.Application verb "Eject" via PowerShell to avoid extra deps.
    // Try Shell.Application with explicit backslash first, then without.
    let ps = format!(
        "$d1='{target}\\\\'; $d2='{target}'; $app=New-Object -ComObject Shell.Application; \
         $item=$app.NameSpace(17).ParseName($d1); if(-not $item){{ $item=$app.NameSpace(17).ParseName($d2) }}; \
         if($item){{ $item.InvokeVerb('Eject'); exit 0 }} else {{ exit 1 }}"
    );
    let status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(ps)
        .status()
        .map_err(|e| {
            FsError::new(
                FsErrorCode::TaskFailed,
                format!("Failed to spawn PowerShell: {e}"),
            )
        })?;
    if status.success() {
        return Ok(());
    }

    // Fallback #1: try to dismount using mountvol /p
    let mv_status = Command::new("mountvol")
        .arg(format!("{target}\\"))
        .arg("/p")
        .status()
        .map_err(|e| {
            FsError::new(
                FsErrorCode::TaskFailed,
                format!("Failed to run mountvol: {e}"),
            )
        })?;
    if mv_status.success() {
        return Ok(());
    }

    // Fallback #2: PowerShell Dismount-Volume -Force (may still fail if locked)
    let ps_force = format!(
        "$d='{target}'; $dl=$d.TrimEnd(':'); try {{ Dismount-Volume -DriveLetter $dl -Force -Confirm:$false; exit 0 }} catch {{ exit 1 }}"
    );
    let force_status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(ps_force)
        .status()
        .map_err(|e| {
            FsError::new(
                FsErrorCode::TaskFailed,
                format!("Failed to spawn PowerShell: {e}"),
            )
        })?;
    if force_status.success() {
        return Ok(());
    }

    Err(FsError::new(
        FsErrorCode::TaskFailed,
        format!("Failed to eject drive {target}"),
    ))
}

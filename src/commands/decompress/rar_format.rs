use std::{
    ffi::CString,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

#[cfg(any(target_os = "linux", target_os = "netbsd"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use unrar_sys as unrar;
use zeroize::Zeroizing;

use super::error::{DecompressError, DecompressResult};
use super::util::ScanControl;
use super::util::{
    check_cancel, clean_relative_path, ensure_dir_nofollow, first_component, open_unique_file,
    path_exists_nofollow, restore_file_mode, CreatedPaths, ExtractBudget, ProgressEmitter,
    SkipStats, CHUNK, EXTRACT_TOTAL_ENTRIES_CAP,
};

#[derive(Clone, Debug)]
pub(super) struct RarEntry {
    name: PathBuf,
    length: u64,
    is_dir: bool,
    mode: Option<u32>,
}

struct RarPassword(Zeroizing<Vec<unrar::WCHAR>>);

impl RarPassword {
    fn new(password: Option<&str>) -> DecompressResult<Self> {
        let password = password.unwrap_or_default();
        #[cfg(windows)]
        let wide: Vec<unrar::WCHAR> = password.encode_utf16().collect();
        #[cfg(not(windows))]
        let wide: Vec<unrar::WCHAR> = password.chars().map(|c| c as unrar::WCHAR).collect();
        if wide.len() >= 512 || password.contains('\0') {
            return Err("RAR password is too long or contains NUL characters".into());
        }
        Ok(Self(Zeroizing::new(wide)))
    }

    fn supply(&self, message: unrar::UINT, buffer: unrar::LPARAM, capacity: unrar::LPARAM) -> i32 {
        if message != unrar::UCM_NEEDPASSWORDW
            || buffer == 0
            || capacity <= 0
            || self.0.is_empty()
            || self.0.len() >= capacity as usize
        {
            return -1;
        }
        // UnRAR owns this writable WCHAR buffer for the duration of its callback.
        unsafe {
            let output = buffer as *mut unrar::WCHAR;
            std::ptr::copy_nonoverlapping(self.0.as_ptr(), output, self.0.len());
            output.add(self.0.len()).write(0);
        }
        1
    }
}

extern "C" fn password_callback(
    message: unrar::UINT,
    data: unrar::LPARAM,
    p1: unrar::LPARAM,
    p2: unrar::LPARAM,
) -> i32 {
    if message == unrar::UCM_CHANGEVOLUME || message == unrar::UCM_CHANGEVOLUMEW {
        return if p2 == unrar::RAR_VOL_NOTIFY { 1 } else { -1 };
    }
    if data == 0 {
        return -1;
    }
    // The boxed password outlives the archive handle, including its close callback.
    unsafe { (&*(data as *const RarPassword)).supply(message, p1, p2) }
}

struct RarArchive(*const unrar::Handle, Box<RarPassword>);

impl Drop for RarArchive {
    fn drop(&mut self) {
        unsafe { unrar::RARCloseArchive(self.0) };
    }
}

impl RarArchive {
    #[allow(clippy::unnecessary_mut_passed)] // unrar_sys declares C output buffers as `*const`.
    fn open(path: &Path, mode: unrar::UINT, password: Option<&str>) -> DecompressResult<Self> {
        let password = Box::new(RarPassword::new(password)?);
        let password_ptr = &*password as *const RarPassword as unrar::LPARAM;
        #[cfg(windows)]
        let (handle, result) = {
            let wide: Vec<unrar::WCHAR> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mut data = unrar::OpenArchiveDataEx::new(wide.as_ptr(), mode);
            data.callback = Some(password_callback);
            data.user_data = password_ptr;
            (
                unsafe { unrar::RAROpenArchiveEx(&mut data) },
                data.open_result as i32,
            )
        };
        #[cfg(any(target_os = "linux", target_os = "netbsd"))]
        let (handle, result) = {
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
                DecompressError::from_external_message(
                    "RAR archive path contains an embedded NUL byte",
                )
            })?;
            let mut data = unrar::OpenArchiveDataEx::new(path.as_ptr(), mode);
            data.callback = Some(password_callback);
            data.user_data = password_ptr;
            (
                unsafe { unrar::RAROpenArchiveEx(&mut data) },
                data.open_result as i32,
            )
        };
        #[cfg(all(not(windows), not(any(target_os = "linux", target_os = "netbsd"))))]
        let (handle, result) = {
            let path = path.to_str().ok_or_else(|| {
                DecompressError::from_external_message(
                    "RAR archive path is not valid Unicode on this platform",
                )
            })?;
            let wide: Vec<unrar::WCHAR> = path
                .chars()
                .map(|character| character as unrar::WCHAR)
                .chain(std::iter::once(0))
                .collect();
            let mut data = unrar::OpenArchiveDataEx::new(wide.as_ptr(), mode);
            data.callback = Some(password_callback);
            data.user_data = password_ptr;
            (
                unsafe { unrar::RAROpenArchiveEx(&mut data) },
                data.open_result as i32,
            )
        };
        if handle.is_null() || result != unrar::ERAR_SUCCESS {
            if !handle.is_null() {
                unsafe { unrar::RARCloseArchive(handle) };
            }
            if result == unrar::ERAR_BAD_DATA && !password.0.is_empty() {
                return Err(DecompressError::password_or_corrupt());
            }
            return Err(unrar_error("open RAR archive", result));
        }
        Ok(Self(handle, password))
    }

    #[allow(clippy::unnecessary_mut_passed)] // unrar_sys declares C output buffers as `*const`.
    fn read_header(&self) -> DecompressResult<Option<RarEntry>> {
        let mut header = unrar::HeaderDataEx::default();
        match unsafe { unrar::RARReadHeaderEx(self.0, &mut header) } {
            unrar::ERAR_SUCCESS => Ok(Some(RarEntry {
                name: header_path(&header),
                length: unpack_size(header.unp_size, header.unp_size_high),
                is_dir: header.flags & unrar::RHDF_DIRECTORY != 0,
                mode: (header.host_os == 3).then_some(header.file_attr),
            })),
            unrar::ERAR_END_ARCHIVE => Ok(None),
            unrar::ERAR_BAD_DATA if !self.1 .0.is_empty() => {
                Err(DecompressError::password_or_corrupt())
            }
            code => Err(unrar_error("read RAR header", code)),
        }
    }

    fn skip_entry(&self) -> DecompressResult<()> {
        let result = unsafe {
            unrar::RARProcessFile(self.0, unrar::RAR_SKIP, std::ptr::null(), std::ptr::null())
        };
        (result == unrar::ERAR_SUCCESS)
            .then_some(())
            .ok_or_else(|| unrar_error("skip RAR entry", result))
    }

    fn stream_entry(&self, state: &mut StreamState<'_>) -> DecompressResult<()> {
        unsafe {
            unrar::RARSetCallback(
                self.0,
                Some(stream_callback),
                state as *mut _ as unrar::LPARAM,
            );
        }
        let result = unsafe {
            unrar::RARProcessFile(self.0, unrar::RAR_TEST, std::ptr::null(), std::ptr::null())
        };
        unsafe {
            unrar::RARSetCallback(
                self.0,
                Some(password_callback),
                &*self.1 as *const RarPassword as unrar::LPARAM,
            )
        };
        if let Some(error) = state.failure.take() {
            return Err(error);
        }
        if result != unrar::ERAR_SUCCESS {
            if result == unrar::ERAR_BAD_DATA && !self.1 .0.is_empty() {
                return Err(DecompressError::password_or_corrupt());
            }
            return Err(unrar_error("extract RAR entry", result));
        }
        state.writer.flush().map_err(|error| {
            DecompressError::from_external_message(format!(
                "Failed to flush extracted RAR entry {}: {error}",
                state.raw_name
            ))
        })
    }
}

struct StreamState<'a> {
    password: &'a RarPassword,
    writer: &'a mut BufWriter<File>,
    raw_name: &'a str,
    progress: Option<&'a ProgressEmitter>,
    cancel: Option<&'a AtomicBool>,
    budget: &'a ExtractBudget,
    failure: Option<DecompressError>,
}

extern "C" fn stream_callback(
    message: unrar::UINT,
    user_data: unrar::LPARAM,
    p1: unrar::LPARAM,
    p2: unrar::LPARAM,
) -> i32 {
    if message == unrar::UCM_CHANGEVOLUME || message == unrar::UCM_CHANGEVOLUMEW {
        return if p2 == unrar::RAR_VOL_NOTIFY { 1 } else { -1 };
    }
    if user_data == 0 || message != unrar::UCM_PROCESSDATA {
        if user_data != 0
            && (message == unrar::UCM_NEEDPASSWORD || message == unrar::UCM_NEEDPASSWORDW)
        {
            return unsafe {
                (&*(user_data as *const StreamState<'_>))
                    .password
                    .supply(message, p1, p2)
            };
        }
        return 0;
    }
    let state = unsafe { &mut *(user_data as *mut StreamState<'_>) };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if p2 < 0 || (p1 == 0 && p2 != 0) {
            return Err(DecompressError::from_external_message(
                "RAR decoder returned an invalid data block",
            ));
        }
        let bytes = unsafe { std::slice::from_raw_parts(p1 as *const u8, p2 as usize) };
        check_cancel(state.cancel).map_err(|error| {
            DecompressError::from_external_message(format!("Extraction cancelled: {error}"))
        })?;
        state
            .budget
            .reserve_bytes(bytes.len() as u64)
            .map_err(|error| {
                DecompressError::from_external_message(format!(
                    "Extraction size cap exceeded while writing {}: {error}",
                    state.raw_name
                ))
            })?;
        state.writer.write_all(bytes).map_err(|error| {
            DecompressError::from_external_message(format!(
                "Failed to write RAR entry {}: {error}",
                state.raw_name
            ))
        })?;
        if let Some(progress) = state.progress {
            progress.add(bytes.len() as u64);
        }
        Ok(())
    }));
    match result {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            state.failure = Some(error);
            -1
        }
        Err(_) => {
            state.failure = Some(DecompressError::from_external_message(
                "RAR extraction callback panicked",
            ));
            -1
        }
    }
}

pub(super) fn single_root_in_rar(
    path: &Path,
    control: ScanControl<'_>,
) -> DecompressResult<Option<PathBuf>> {
    control.check()?;
    let entries = parse_rar_entries(path, control)?;
    let mut root = None;
    for entry in entries {
        let clean = match clean_relative_path(&entry.name) {
            Ok(path) => path,
            Err(_) => continue,
        };
        if clean.as_os_str().is_empty() {
            continue;
        }
        let Some(first) = first_component(&clean) else {
            continue;
        };
        if !entry.is_dir && clean.components().count() == 1 {
            return Ok(None);
        }
        match &root {
            Some(current) if current != &first => return Ok(None),
            None => root = Some(first),
            _ => {}
        }
    }
    Ok(root)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_rar(
    archive_path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
    password: Option<&str>,
) -> DecompressResult<()> {
    let archive = RarArchive::open(archive_path, unrar::RAR_OM_EXTRACT, password)?;
    while let Some(entry) = archive.read_header()? {
        check_cancel(cancel).map_err(|error| {
            DecompressError::from_external_message(format!("Extraction cancelled: {error}"))
        })?;
        budget.reserve_entry(1).map_err(|error| {
            DecompressError::from_external_message(format!(
                "Extraction entry cap exceeded: {error}"
            ))
        })?;
        let raw_name = entry.name.to_string_lossy().into_owned();
        let clean = match clean_relative_path(&entry.name) {
            Ok(path) => path,
            Err(error) => {
                stats.skip_unsupported(&raw_name, &error.to_string());
                archive.skip_entry()?;
                continue;
            }
        };
        let clean = strip_prefix
            .and_then(|prefix| clean.strip_prefix(prefix).ok().map(Path::to_path_buf))
            .unwrap_or(clean);
        if clean.as_os_str().is_empty() {
            if entry.is_dir {
                created.defer_directory_mode(dest_dir.to_path_buf(), entry.mode);
            }
            archive.skip_entry()?;
            continue;
        }
        let dest_path = dest_dir.join(clean);
        if entry.is_dir {
            created.defer_directory_mode(dest_path.clone(), entry.mode);
            match ensure_dir_nofollow(&dest_path) {
                Ok(dirs) => dirs.into_iter().for_each(|dir| created.record_dir(dir)),
                Err(error) => {
                    stats.skip_unsupported(&raw_name, &format!("create dir failed: {error}"))
                }
            }
            archive.skip_entry()?;
            continue;
        }
        match path_exists_nofollow(&dest_path) {
            Ok(true) => {
                if let Some(progress) = progress {
                    progress.add(entry.length.max(1));
                }
                archive.skip_entry()?;
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                stats.skip_unsupported(&raw_name, &format!("stat destination failed: {error}"));
                archive.skip_entry()?;
                continue;
            }
        }
        if let Some(parent) = dest_path.parent() {
            match ensure_dir_nofollow(parent) {
                Ok(dirs) => dirs.into_iter().for_each(|dir| created.record_dir(dir)),
                Err(error) => {
                    stats.skip_unsupported(&raw_name, &format!("create parent failed: {error}"));
                    archive.skip_entry()?;
                    continue;
                }
            }
        }
        let (file, actual_path) = open_unique_file(&dest_path)?;
        created.record_file(actual_path);
        let mut writer = BufWriter::with_capacity(CHUNK, file);
        archive.stream_entry(&mut StreamState {
            password: &archive.1,
            writer: &mut writer,
            raw_name: &raw_name,
            progress,
            cancel,
            budget,
            failure: None,
        })?;
        restore_file_mode(writer.get_ref(), entry.mode)?;
    }
    Ok(())
}

pub(super) fn parse_rar_entries(
    path: &Path,
    control: ScanControl<'_>,
) -> DecompressResult<Vec<RarEntry>> {
    control.check()?;
    // Listing mode excludes continuation headers of split entries, so a
    // multi-volume archive contributes one logical entry per file.
    let archive = RarArchive::open(path, unrar::RAR_OM_LIST, control.password)?;
    let mut entries = Vec::new();
    while let Some(entry) = archive.read_header()? {
        control.check()?;
        if entries.len() as u64 >= EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(DecompressError::from_external_message(format!(
                "Archive exceeds entry cap (more than {} entries)",
                EXTRACT_TOTAL_ENTRIES_CAP
            )));
        }
        entries.push(entry);
        archive.skip_entry()?;
    }
    Ok(entries)
}

pub(super) fn rar_uncompressed_total_from_entries(entries: &[RarEntry]) -> DecompressResult<u64> {
    Ok(entries
        .iter()
        .fold(0u64, |total, entry| total.saturating_add(entry.length)))
}

fn unpack_size(low: u32, high: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

#[cfg(windows)]
fn header_path(header: &unrar::HeaderDataEx) -> PathBuf {
    let end = header
        .filename_w
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(header.filename_w.len());
    PathBuf::from(std::ffi::OsString::from_wide(&header.filename_w[..end]))
}

#[cfg(not(windows))]
fn header_path(header: &unrar::HeaderDataEx) -> PathBuf {
    let end = header
        .filename_w
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(header.filename_w.len());
    PathBuf::from(
        header.filename_w[..end]
            .iter()
            .map(|&c| char::from_u32(c as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>(),
    )
}

fn unrar_error(context: &str, code: i32) -> DecompressError {
    let detail = match code {
        unrar::ERAR_MISSING_PASSWORD => return DecompressError::password_required(),
        unrar::ERAR_BAD_PASSWORD => return DecompressError::invalid_password(),
        unrar::ERAR_BAD_ARCHIVE => "invalid RAR archive",
        unrar::ERAR_BAD_DATA => "corrupt RAR entry data",
        unrar::ERAR_UNKNOWN_FORMAT => "unsupported RAR format",
        unrar::ERAR_EOPEN => "failed to open archive or a required volume",
        unrar::ERAR_EREAD => "failed to read archive data",
        unrar::ERAR_EWRITE => "decoder failed while writing entry data",
        _ => "UnRAR decoder failed",
    };
    DecompressError::from_external_message(format!("Failed to {context}: {detail} (code {code})"))
}

#[cfg(test)]
mod tests {
    use super::ScanControl;
    use super::{extract_rar, parse_rar_entries, single_root_in_rar};
    use crate::commands::decompress::util::{CreatedPaths, ExtractBudget, SkipStats};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/rar")
            .join(name)
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = format!(
            "browsey-rar-format-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn extracts_decoded_rar_entry_through_browsey_file_guard() {
        let archive = fixture("version.rar");
        let entries =
            parse_rar_entries(&archive, ScanControl::default()).expect("list RAR entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, PathBuf::from("VERSION"));
        assert_eq!(entries[0].length, 11);
        assert_eq!(
            single_root_in_rar(&archive, ScanControl::default()).expect("single root"),
            None
        );

        let root = unique_temp_dir("extract");
        let output = root.join("out");
        fs::create_dir_all(&output).expect("create output");
        let mut created = CreatedPaths::default();
        extract_rar(
            &archive,
            &output,
            None,
            &SkipStats::default(),
            None,
            &mut created,
            None,
            &ExtractBudget::new(1_000_000, 100),
            None,
        )
        .expect("extract RAR");
        assert_eq!(
            fs::read_to_string(output.join("VERSION")).expect("read extracted file"),
            "unrar-0.4.0"
        );
        created.disarm();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preserves_unicode_entry_names() {
        let archive = fixture("unicode-entry.rar");
        let entries =
            parse_rar_entries(&archive, ScanControl::default()).expect("list RAR entries");
        assert_eq!(entries[0].name, PathBuf::from("unicodefilename❤️.txt"));

        let root = unique_temp_dir("unicode");
        let output = root.join("out");
        fs::create_dir_all(&output).expect("create output");
        let mut created = CreatedPaths::default();
        extract_rar(
            &archive,
            &output,
            None,
            &SkipStats::default(),
            None,
            &mut created,
            None,
            &ExtractBudget::new(1_000_000, 100),
            None,
        )
        .expect("extract RAR");
        assert_eq!(
            fs::read_to_string(output.join("unicodefilename❤️.txt")).expect("read unicode file"),
            "foobar\n"
        );
        created.disarm();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_password_requirement_without_writing_output() {
        let archive = fixture("crypted.rar");
        let root = unique_temp_dir("password");
        let output = root.join("out");
        fs::create_dir_all(&output).expect("create output");
        let mut created = CreatedPaths::default();
        let error = extract_rar(
            &archive,
            &output,
            None,
            &SkipStats::default(),
            None,
            &mut created,
            None,
            &ExtractBudget::new(1_000_000, 100),
            None,
        )
        .expect_err("encrypted archive must fail without a password");
        assert!(error.to_string().contains("requires a password"));
        drop(created);
        assert!(fs::read_dir(&output).expect("read output").next().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_compressed_rar5_entry() {
        let archive = fixture("rar5-compressed.rar");
        let entries =
            parse_rar_entries(&archive, ScanControl::default()).expect("list RAR5 entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, PathBuf::from("test.bin"));
        assert_eq!(entries[0].length, 1_200);

        let root = unique_temp_dir("rar5");
        let output = root.join("out");
        fs::create_dir_all(&output).expect("create output");
        let mut created = CreatedPaths::default();
        extract_rar(
            &archive,
            &output,
            None,
            &SkipStats::default(),
            None,
            &mut created,
            None,
            &ExtractBudget::new(1_000_000, 100),
            None,
        )
        .expect("extract RAR5");
        let bytes = fs::read(output.join("test.bin")).expect("read extracted RAR5 file");
        assert_eq!(bytes.len(), 1_200);
        let (words, remainder) = bytes.as_chunks::<4>();
        assert!(remainder.is_empty());
        for (index, word) in words.iter().enumerate() {
            let number = (index + 1) as i32;
            let expected = (number * number - 3 * number + 1).max(0) as u32;
            assert_eq!(u32::from_le_bytes(*word), expected);
        }
        created.disarm();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_rar5_multi_volume_archive() {
        let archive = fixture("test_read_format_rar5_multiarchive.part01.rar");
        let entries = parse_rar_entries(&archive, ScanControl::default())
            .expect("list multi-volume RAR5 entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].name,
            PathBuf::from("home/antek/temp/build/unrar5/libarchive/bin/bsdcat_test")
        );
        assert_eq!(
            entries[1].name,
            PathBuf::from("home/antek/temp/build/unrar5/libarchive/bin/bsdtar_test")
        );

        let root = unique_temp_dir("rar5-multi");
        let output = root.join("out");
        fs::create_dir_all(&output).expect("create output");
        let mut created = CreatedPaths::default();
        extract_rar(
            &archive,
            &output,
            None,
            &SkipStats::default(),
            None,
            &mut created,
            None,
            &ExtractBudget::new(1_000_000, 100),
            None,
        )
        .expect("extract multi-volume RAR5");
        assert_eq!(
            fs::read(output.join("home/antek/temp/build/unrar5/libarchive/bin/bsdcat_test"))
                .expect("read first multi-volume file")
                .len(),
            144_608
        );
        assert_eq!(
            fs::read(output.join("home/antek/temp/build/unrar5/libarchive/bin/bsdtar_test"))
                .expect("read second multi-volume file")
                .len(),
            365_672
        );
        created.disarm();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_incomplete_multi_volume_archive_without_output() {
        let root = unique_temp_dir("rar5-missing-volume");
        let archive = root.join("incomplete.part01.rar");
        fs::copy(
            fixture("test_read_format_rar5_multiarchive.part01.rar"),
            &archive,
        )
        .expect("copy first volume");
        let output = root.join("out");
        fs::create_dir_all(&output).expect("create output");
        let mut created = CreatedPaths::default();
        let error = extract_rar(
            &archive,
            &output,
            None,
            &SkipStats::default(),
            None,
            &mut created,
            None,
            &ExtractBudget::new(1_000_000, 100),
            None,
        )
        .expect_err("incomplete multi-volume archive must fail");
        assert!(error.to_string().contains("required volume"));
        drop(created);
        assert!(fs::read_dir(&output).expect("read output").next().is_none());
        let _ = fs::remove_dir_all(root);
    }
}

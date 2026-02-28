use super::super::error::{map_external_result, FsError, FsErrorCode, FsResult};
use super::backend::TrashBackend;
use crate::undo::{
    assert_path_snapshot, is_destination_exists_error, move_with_fallback, PathSnapshot,
};
#[cfg(not(target_os = "windows"))]
use ::trash::TrashItem;
#[cfg(not(target_os = "windows"))]
use std::ffi::OsString;
#[cfg(not(target_os = "windows"))]
use std::fmt::Write as _;
#[cfg(not(target_os = "windows"))]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "windows"))]
use std::sync::{Mutex, OnceLock};
#[cfg(not(target_os = "windows"))]
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrashStageJournalEntry {
    pub(super) staged: PathBuf,
    pub(super) original: PathBuf,
}

#[cfg(not(target_os = "windows"))]
fn trash_stage_journal_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join("trash-stage-journal.tsv")
}

#[cfg(not(target_os = "windows"))]
fn trash_stage_journal_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(not(target_os = "windows"))]
fn stage_for_trash(src: &Path) -> Result<PathBuf, String> {
    let parent = src
        .parent()
        .ok_or_else(|| format!("Invalid source path: {}", src.display()))?;
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    for attempt in 0..64u32 {
        let staged = parent.join(format!("browsey-trash-stage-{pid}-{seed}-{attempt}"));
        match move_with_fallback(src, &staged) {
            Ok(_) => return Ok(staged),
            Err(err) if is_destination_exists_error(&err) => continue,
            Err(err) => {
                return Err(format!(
                    "Failed to stage {} for trash: {err}",
                    src.display()
                ));
            }
        }
    }
    Err(format!(
        "Failed to allocate a temporary staged path for {}",
        src.display()
    ))
}

pub(super) fn trash_delete_via_staged_rename<B: TrashBackend>(
    src: &Path,
    src_snapshot: &PathSnapshot,
    backend: &B,
) -> FsResult<PathBuf> {
    map_external_result(assert_path_snapshot(src, src_snapshot))?;
    #[cfg(target_os = "windows")]
    {
        backend.delete_path(src)?;
        Ok(src.to_path_buf())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let staged = map_external_result(stage_for_trash(src))?;
        if let Err(err) = add_trash_stage_journal_entry(&staged, src) {
            let rollback = move_with_fallback(&staged, src)
                .err()
                .map(|e| format!("; rollback failed: {e}"))
                .unwrap_or_default();
            return Err(FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to persist staged trash journal entry: {err}{rollback}"),
            ));
        }
        match backend.delete_path(&staged) {
            Ok(_) => {
                let _ = remove_trash_stage_journal_entry(&staged, src);
                Ok(staged)
            }
            Err(err) => {
                let rollback_result = move_with_fallback(&staged, src);
                if rollback_result.is_ok() {
                    let _ = remove_trash_stage_journal_entry(&staged, src);
                }
                let rollback = rollback_result
                    .err()
                    .map(|e| format!("; rollback failed: {e}"))
                    .unwrap_or_default();
                Err(FsError::new(
                    FsErrorCode::TrashFailed,
                    format!("Failed to move to trash: {err}{rollback}"),
                ))
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn percent_encode_trash_info_segment(segment: &[u8], out: &mut String) {
    for byte in segment {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn encode_trash_info_path(path: &Path) -> String {
    let bytes = path.as_os_str().as_bytes();
    let mut out = String::with_capacity(bytes.len().saturating_mul(3).max(1));

    if bytes.starts_with(b"/") {
        out.push('/');
    }
    let start = usize::from(bytes.starts_with(b"/"));
    let mut first_segment = true;
    for segment in bytes[start..].split(|b| *b == b'/') {
        if segment.is_empty() {
            continue;
        }
        if !first_segment {
            out.push('/');
        }
        percent_encode_trash_info_segment(segment, &mut out);
        first_segment = false;
    }
    if out.is_empty() {
        ".".to_string()
    } else {
        out
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn decode_percent_encoded_unix_path(encoded: &str) -> FsResult<PathBuf> {
    fn hex_val(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(FsError::new(
                    FsErrorCode::InvalidPath,
                    format!("Invalid percent encoding: '{encoded}'"),
                ));
            }
            let hi = hex_val(bytes[i + 1]).ok_or_else(|| {
                FsError::new(
                    FsErrorCode::InvalidPath,
                    format!(
                        "Invalid percent encoding at index {} in '{}'",
                        i + 1,
                        encoded
                    ),
                )
            })?;
            let lo = hex_val(bytes[i + 2]).ok_or_else(|| {
                FsError::new(
                    FsErrorCode::InvalidPath,
                    format!(
                        "Invalid percent encoding at index {} in '{}'",
                        i + 2,
                        encoded
                    ),
                )
            })?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(PathBuf::from(OsString::from_vec(out)))
}

#[cfg(not(target_os = "windows"))]
pub(super) fn load_trash_stage_journal_entries_at(
    path: &Path,
) -> FsResult<Vec<TrashStageJournalEntry>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(FsError::new(
                FsErrorCode::TrashFailed,
                format!(
                    "Failed to read trash stage journal {}: {err}",
                    path.display()
                ),
            ));
        }
    };

    let mut entries = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let staged_enc = match parts.next() {
            Some(v) if !v.is_empty() => v,
            _ => {
                warn!(
                    "Ignoring malformed trash stage journal entry at line {}",
                    idx + 1
                );
                continue;
            }
        };
        let original_enc = match parts.next() {
            Some(v) if !v.is_empty() => v,
            _ => {
                warn!(
                    "Ignoring malformed trash stage journal entry at line {}",
                    idx + 1
                );
                continue;
            }
        };

        let staged = match decode_percent_encoded_unix_path(staged_enc) {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    "Ignoring invalid staged path in trash stage journal at line {}: {}",
                    idx + 1,
                    err
                );
                continue;
            }
        };
        let original = match decode_percent_encoded_unix_path(original_enc) {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    "Ignoring invalid original path in trash stage journal at line {}: {}",
                    idx + 1,
                    err
                );
                continue;
            }
        };
        entries.push(TrashStageJournalEntry { staged, original });
    }

    Ok(entries)
}

#[cfg(not(target_os = "windows"))]
fn load_trash_stage_journal_entries() -> FsResult<Vec<TrashStageJournalEntry>> {
    load_trash_stage_journal_entries_at(&trash_stage_journal_path())
}

#[cfg(not(target_os = "windows"))]
pub(super) fn store_trash_stage_journal_entries_at(
    path: &Path,
    entries: &[TrashStageJournalEntry],
) -> FsResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!(
                    "Failed to create trash stage journal directory {}: {e}",
                    parent.display()
                ),
            )
        })?;
    }

    if entries.is_empty() {
        if let Err(err) = std::fs::remove_file(path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                return Err(FsError::new(
                    FsErrorCode::TrashFailed,
                    format!(
                        "Failed to remove trash stage journal {}: {err}",
                        path.display()
                    ),
                ));
            }
        }
        return Ok(());
    }

    let mut content = String::new();
    for entry in entries {
        content.push_str(&encode_trash_info_path(&entry.staged));
        content.push('\t');
        content.push_str(&encode_trash_info_path(&entry.original));
        content.push('\n');
    }

    let tmp_path = path.with_extension("tsv.tmp");
    std::fs::write(&tmp_path, content).map_err(|e| {
        FsError::new(
            FsErrorCode::TrashFailed,
            format!(
                "Failed to write temporary trash stage journal {}: {e}",
                tmp_path.display()
            ),
        )
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        FsError::new(
            FsErrorCode::TrashFailed,
            format!(
                "Failed to finalize trash stage journal {}: {e}",
                path.display()
            ),
        )
    })
}

#[cfg(not(target_os = "windows"))]
fn store_trash_stage_journal_entries(entries: &[TrashStageJournalEntry]) -> FsResult<()> {
    store_trash_stage_journal_entries_at(&trash_stage_journal_path(), entries)
}

#[cfg(not(target_os = "windows"))]
fn add_trash_stage_journal_entry(staged: &Path, original: &Path) -> FsResult<()> {
    let _guard = trash_stage_journal_lock().lock().map_err(|_| {
        FsError::new(
            FsErrorCode::TrashFailed,
            "Trash stage journal lock poisoned",
        )
    })?;
    let mut entries = load_trash_stage_journal_entries()?;
    let new_entry = TrashStageJournalEntry {
        staged: staged.to_path_buf(),
        original: original.to_path_buf(),
    };
    if !entries.contains(&new_entry) {
        entries.push(new_entry);
    }
    store_trash_stage_journal_entries(&entries)
}

#[cfg(not(target_os = "windows"))]
fn remove_trash_stage_journal_entry(staged: &Path, original: &Path) -> FsResult<()> {
    let _guard = trash_stage_journal_lock().lock().map_err(|_| {
        FsError::new(
            FsErrorCode::TrashFailed,
            "Trash stage journal lock poisoned",
        )
    })?;
    let mut entries = load_trash_stage_journal_entries()?;
    entries.retain(|entry| !(entry.staged == staged && entry.original == original));
    store_trash_stage_journal_entries(&entries)
}

pub fn cleanup_stale_trash_staging() {
    #[cfg(target_os = "windows")]
    {
        return;
    }
    #[cfg(not(target_os = "windows"))]
    {
        cleanup_stale_trash_staging_at(&trash_stage_journal_path());
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn cleanup_stale_trash_staging_at(path: &Path) {
    let _guard = match trash_stage_journal_lock().lock() {
        Ok(guard) => guard,
        Err(_) => {
            warn!("Trash stage journal lock poisoned");
            return;
        }
    };

    let entries = match load_trash_stage_journal_entries_at(path) {
        Ok(entries) => entries,
        Err(err) => {
            warn!("{err}");
            return;
        }
    };
    if entries.is_empty() {
        return;
    }

    let mut retained = Vec::new();
    for entry in entries {
        let staged_meta = std::fs::symlink_metadata(&entry.staged);
        if let Err(err) = staged_meta {
            if err.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Failed to inspect staged trash path {}: {}",
                    entry.staged.display(),
                    err
                );
                retained.push(entry);
            }
            continue;
        }

        match move_with_fallback(&entry.staged, &entry.original) {
            Ok(_) => {}
            Err(err) => {
                warn!(
                    "Failed to recover staged trash item {} -> {}: {}",
                    entry.staged.display(),
                    entry.original.display(),
                    err
                );
                retained.push(entry);
            }
        }
    }

    if let Err(err) = store_trash_stage_journal_entries_at(path, &retained) {
        warn!("{err}");
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn rewrite_trash_info_original_path(
    item: &TrashItem,
    original_path: &Path,
) -> FsResult<()> {
    let info_path = PathBuf::from(&item.id);
    let contents = std::fs::read_to_string(&info_path).map_err(|e| {
        FsError::new(
            FsErrorCode::TrashFailed,
            format!(
                "Failed to read trash info file {}: {e}",
                info_path.display()
            ),
        )
    })?;

    let replacement = format!("Path={}", encode_trash_info_path(original_path));
    let mut output = String::with_capacity(contents.len() + replacement.len() + 8);
    let mut replaced = false;
    for line in contents.lines() {
        if line.starts_with("Path=") {
            output.push_str(&replacement);
            output.push('\n');
            replaced = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if !replaced {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&replacement);
        output.push('\n');
    }

    std::fs::write(&info_path, output).map_err(|e| {
        FsError::new(
            FsErrorCode::TrashFailed,
            format!(
                "Failed to update trash info file {}: {e}",
                info_path.display()
            ),
        )
    })
}

use super::{should_abort_fs_op, CancelState, DeleteProgressPayload, DirListing, UndoState};
use crate::{
    entry::{build_entry, FsEntry},
    fs_utils::{check_no_symlink_components, debug_log, sanitize_path_nofollow},
    icons::icon_id_for,
    runtime_lifecycle,
    sorting::{sort_entries, SortSpec},
    undo::{
        assert_path_snapshot, copy_entry as undo_copy_entry, delete_entry_path as undo_delete_path,
        rename_entry_nofollow_io, run_actions, snapshot_existing_path, temp_backup_path, Action,
        Direction, PathSnapshot,
    },
};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
#[cfg(not(target_os = "windows"))]
use std::fmt::Write as _;
#[cfg(not(target_os = "windows"))]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::warn;
use trash::{
    delete as trash_delete,
    os_limited::{list as trash_list, metadata as trash_metadata, purge_all, restore_all},
    TrashItem, TrashItemSize,
};

#[cfg(not(target_os = "windows"))]
fn restorable_file_in_trash_from_info_file(info_file: &Path) -> PathBuf {
    let trash_folder = info_file.parent().and_then(|p| p.parent());
    let name_in_trash = info_file.file_stem();
    match (trash_folder, name_in_trash) {
        (Some(folder), Some(name)) => folder.join("files").join(name),
        _ => PathBuf::from(info_file),
    }
}

fn trash_item_path(item: &TrashItem) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(&item.id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        restorable_file_in_trash_from_info_file(Path::new(&item.id))
    }
}

fn apply_original_trash_fields(
    entry: &mut FsEntry,
    original_path: &Path,
    item: &TrashItem,
    meta: &std::fs::Metadata,
    is_link: bool,
) {
    entry.name = original_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| original_path.to_string_lossy().into_owned());
    entry.ext = original_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    entry.original_path = Some(original_path.to_string_lossy().into_owned());
    entry.trash_id = Some(item.id.to_string_lossy().into_owned());
    entry.icon_id = icon_id_for(original_path, meta, is_link);
}

#[tauri::command]
pub fn list_trash(sort: Option<SortSpec>) -> Result<DirListing, String> {
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let mut entries = Vec::new();
    for item in items {
        let path = trash_item_path(&item);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                debug_log(&format!(
                    "trash list: missing item path={}, skipping: {e:?}",
                    path.display()
                ));
                continue;
            }
        };
        let is_link = meta.file_type().is_symlink();
        let mut entry = build_entry(&path, &meta, is_link, false);
        let original_path = item.original_path();
        apply_original_trash_fields(&mut entry, &original_path, &item, &meta, is_link);
        if let Ok(info) = trash_metadata(&item) {
            match info.size {
                TrashItemSize::Bytes(b) => entry.size = Some(b),
                TrashItemSize::Entries(n) => entry.items = Some(n as u64),
            }
        }
        entries.push(entry);
    }
    sort_entries(&mut entries, sort);
    Ok(DirListing {
        current: "Trash".to_string(),
        entries,
    })
}

#[tauri::command]
pub async fn move_to_trash(
    path: String,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let action = move_single_to_trash(&path, &app_handle, true)?;
        let _ = undo_state.record_applied(action);
        Ok(())
    })
    .await
    .map_err(|e| format!("Move to trash task failed: {e}"))?
}

fn emit_trash_progress(
    app: &tauri::AppHandle,
    event: Option<&String>,
    done: u64,
    total: u64,
    finished: bool,
    last_emit: &mut Instant,
) {
    if let Some(evt) = event {
        let now = Instant::now();
        if finished || now.duration_since(*last_emit) >= Duration::from_millis(100) {
            let payload = DeleteProgressPayload {
                bytes: done,
                total,
                finished,
            };
            let _ = runtime_lifecycle::emit_if_running(app, evt, payload);
            *last_emit = now;
        }
    }
}

struct PreparedTrashMove {
    src: PathBuf,
    backup: PathBuf,
    src_snapshot: PathSnapshot,
    staged_src: Option<PathBuf>,
}

trait TrashBackend {
    fn list_items(&self) -> Result<Vec<TrashItem>, String>;
    fn delete_path(&self, path: &Path) -> Result<(), String>;
    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> Result<(), String>;
}

struct SystemTrashBackend;

impl TrashBackend for SystemTrashBackend {
    fn list_items(&self) -> Result<Vec<TrashItem>, String> {
        trash_list().map_err(|e| format!("Failed to list trash: {e}"))
    }

    fn delete_path(&self, path: &Path) -> Result<(), String> {
        trash_delete(path).map_err(|e| format!("Failed to move to trash: {e}"))
    }

    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> Result<(), String> {
        #[cfg(not(target_os = "windows"))]
        {
            return rewrite_trash_info_original_path(item, original_path);
        }
        #[cfg(target_os = "windows")]
        {
            let _ = (item, original_path);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrashStageJournalEntry {
    staged: PathBuf,
    original: PathBuf,
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
        match rename_entry_nofollow_io(src, &staged) {
            Ok(_) => return Ok(staged),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
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

fn trash_delete_via_staged_rename<B: TrashBackend>(
    src: &Path,
    src_snapshot: &PathSnapshot,
    backend: &B,
) -> Result<PathBuf, String> {
    assert_path_snapshot(src, src_snapshot)?;
    #[cfg(target_os = "windows")]
    {
        backend.delete_path(src)?;
        Ok(src.to_path_buf())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let staged = stage_for_trash(src)?;
        if let Err(err) = add_trash_stage_journal_entry(&staged, src) {
            let rollback = rename_entry_nofollow_io(&staged, src)
                .err()
                .map(|e| format!("; rollback failed: {e}"))
                .unwrap_or_default();
            return Err(format!(
                "Failed to persist staged trash journal entry: {err}{rollback}"
            ));
        }
        match backend.delete_path(&staged) {
            Ok(_) => {
                let _ = remove_trash_stage_journal_entry(&staged, src);
                Ok(staged)
            }
            Err(err) => {
                let rollback_result = rename_entry_nofollow_io(&staged, src);
                if rollback_result.is_ok() {
                    let _ = remove_trash_stage_journal_entry(&staged, src);
                }
                let rollback = rollback_result
                    .err()
                    .map(|e| format!("; rollback failed: {e}"))
                    .unwrap_or_default();
                Err(format!("Failed to move to trash: {err}{rollback}"))
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
fn encode_trash_info_path(path: &Path) -> String {
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
fn decode_percent_encoded_unix_path(encoded: &str) -> Result<PathBuf, String> {
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
                return Err(format!("Invalid percent encoding: '{encoded}'"));
            }
            let hi = hex_val(bytes[i + 1]).ok_or_else(|| {
                format!(
                    "Invalid percent encoding at index {} in '{}'",
                    i + 1,
                    encoded
                )
            })?;
            let lo = hex_val(bytes[i + 2]).ok_or_else(|| {
                format!(
                    "Invalid percent encoding at index {} in '{}'",
                    i + 2,
                    encoded
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
fn load_trash_stage_journal_entries_at(path: &Path) -> Result<Vec<TrashStageJournalEntry>, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(format!(
                "Failed to read trash stage journal {}: {err}",
                path.display()
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
fn load_trash_stage_journal_entries() -> Result<Vec<TrashStageJournalEntry>, String> {
    load_trash_stage_journal_entries_at(&trash_stage_journal_path())
}

#[cfg(not(target_os = "windows"))]
fn store_trash_stage_journal_entries_at(
    path: &Path,
    entries: &[TrashStageJournalEntry],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create trash stage journal directory {}: {e}",
                parent.display()
            )
        })?;
    }

    if entries.is_empty() {
        if let Err(err) = std::fs::remove_file(path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                return Err(format!(
                    "Failed to remove trash stage journal {}: {err}",
                    path.display()
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
        format!(
            "Failed to write temporary trash stage journal {}: {e}",
            tmp_path.display()
        )
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        format!(
            "Failed to finalize trash stage journal {}: {e}",
            path.display()
        )
    })
}

#[cfg(not(target_os = "windows"))]
fn store_trash_stage_journal_entries(entries: &[TrashStageJournalEntry]) -> Result<(), String> {
    store_trash_stage_journal_entries_at(&trash_stage_journal_path(), entries)
}

#[cfg(not(target_os = "windows"))]
fn add_trash_stage_journal_entry(staged: &Path, original: &Path) -> Result<(), String> {
    let _guard = trash_stage_journal_lock()
        .lock()
        .map_err(|_| "Trash stage journal lock poisoned".to_string())?;
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
fn remove_trash_stage_journal_entry(staged: &Path, original: &Path) -> Result<(), String> {
    let _guard = trash_stage_journal_lock()
        .lock()
        .map_err(|_| "Trash stage journal lock poisoned".to_string())?;
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
fn cleanup_stale_trash_staging_at(path: &Path) {
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

        match rename_entry_nofollow_io(&entry.staged, &entry.original) {
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
fn rewrite_trash_info_original_path(item: &TrashItem, original_path: &Path) -> Result<(), String> {
    let info_path = PathBuf::from(&item.id);
    let contents = std::fs::read_to_string(&info_path).map_err(|e| {
        format!(
            "Failed to read trash info file {}: {e}",
            info_path.display()
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
        format!(
            "Failed to update trash info file {}: {e}",
            info_path.display()
        )
    })
}

fn rollback_prepared_trash(prepared: &[PreparedTrashMove]) {
    let mut rollback: Vec<Action> = prepared
        .iter()
        .map(|p| Action::Delete {
            path: p.src.clone(),
            backup: p.backup.clone(),
        })
        .collect();
    let _ = run_actions(&mut rollback, Direction::Backward);
}

fn prepare_trash_move(raw: &str) -> Result<PreparedTrashMove, String> {
    let src = sanitize_path_nofollow(raw, true)?;
    check_no_symlink_components(&src)?;
    let src_snapshot = snapshot_existing_path(&src)?;

    // Backup into the central undo directory in case we cannot locate the trash item path later.
    let backup = temp_backup_path(&src);
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
    }
    undo_copy_entry(&src, &backup)?;
    if let Err(err) = assert_path_snapshot(&src, &src_snapshot) {
        let _ = undo_delete_path(&backup);
        return Err(err);
    }

    Ok(PreparedTrashMove {
        src,
        backup,
        src_snapshot,
        staged_src: None,
    })
}

fn move_to_trash_many_with_backend<B, FShouldAbort, FEmitProgress, FEmitChanged>(
    paths: Vec<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
    backend: &B,
    mut should_abort: FShouldAbort,
    mut emit_progress: FEmitProgress,
    mut emit_changed: FEmitChanged,
) -> Result<(), String>
where
    B: TrashBackend,
    FShouldAbort: FnMut(Option<&AtomicBool>) -> bool,
    FEmitProgress: FnMut(u64, u64, bool),
    FEmitChanged: FnMut(),
{
    let total = paths.len() as u64;
    if total == 0 {
        emit_progress(0, 0, true);
        return Ok(());
    }
    // Capture current trash contents once to avoid O(n^2) directory scans.
    let before_ids: HashSet<OsString> = backend
        .list_items()?
        .into_iter()
        .map(|item| item.id)
        .collect();

    let mut prepared: Vec<PreparedTrashMove> = Vec::with_capacity(paths.len());
    let mut done: u64 = 0;
    for path in paths {
        if should_abort(cancel) {
            rollback_prepared_trash(&prepared);
            emit_progress(done, total, true);
            return Err("Move to trash cancelled".into());
        }
        match prepare_trash_move(&path) {
            Ok(mut prep) => {
                match trash_delete_via_staged_rename(&prep.src, &prep.src_snapshot, backend) {
                    Ok(staged_src) => {
                        prep.staged_src = Some(staged_src);
                        done = done.saturating_add(1);
                        emit_progress(done, total, done == total);
                        prepared.push(prep);
                    }
                    Err(err) => {
                        rollback_prepared_trash(&prepared);
                        let _ = undo_delete_path(&prep.backup);
                        emit_progress(done, total, true);
                        return Err(err);
                    }
                }
            }
            Err(err) => {
                // Nothing was moved for this entry; roll back previous ones.
                rollback_prepared_trash(&prepared);
                emit_progress(done, total, true);
                return Err(err);
            }
        }
    }

    // Identify new trash items with a single post-scan.
    let mut new_items: HashMap<PathBuf, TrashItem> = HashMap::new();
    if let Ok(after) = backend.list_items() {
        for item in after.into_iter().filter(|i| !before_ids.contains(&i.id)) {
            new_items.insert(item.original_path(), item);
        }
    }

    let mut actions = Vec::with_capacity(prepared.len());
    for prep in prepared {
        let lookup = prep.staged_src.as_ref().unwrap_or(&prep.src);
        if let Some(item) = new_items.remove(lookup) {
            if let Err(err) = backend.rewrite_original_path(&item, &prep.src) {
                warn!(
                    "Failed to rewrite trash info for {}: {}",
                    prep.src.display(),
                    err
                );
            }
            let _ = undo_delete_path(&prep.backup);
            actions.push(Action::Move {
                from: prep.src,
                to: trash_item_path(&item),
            });
        } else {
            actions.push(Action::Delete {
                path: prep.src,
                backup: prep.backup,
            });
        }
    }

    let recorded = if actions.len() == 1 {
        actions.pop().unwrap()
    } else {
        Action::Batch(actions)
    };
    let _ = undo.record_applied(recorded);
    emit_changed();
    Ok(())
}

fn move_to_trash_many_blocking(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: UndoState,
    progress_event: Option<String>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let backend = SystemTrashBackend;
    let mut last_emit = Instant::now();
    move_to_trash_many_with_backend(
        paths,
        undo,
        cancel,
        &backend,
        |cancel_flag| should_abort_fs_op(&app, cancel_flag),
        |done, total, finished| {
            emit_trash_progress(
                &app,
                progress_event.as_ref(),
                done,
                total,
                finished,
                &mut last_emit,
            );
        },
        || {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        },
    )
}

#[tauri::command]
pub async fn move_to_trash_many(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    let cancel_state = cancel.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cancel_guard = progress_event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        move_to_trash_many_blocking(
            paths,
            app_handle,
            undo_state,
            progress_event,
            cancel_token.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Move to trash task failed: {e}"))?
}

fn move_single_to_trash_with_backend<B: TrashBackend>(
    path: &str,
    backend: &B,
) -> Result<Action, String> {
    let src = sanitize_path_nofollow(path, true)?;
    check_no_symlink_components(&src)?;
    let src_snapshot = snapshot_existing_path(&src)?;

    // Backup into the central undo directory in case the OS trash item can't be found.
    let backup = temp_backup_path(&src);
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
    }
    undo_copy_entry(&src, &backup)?;

    let before: HashSet<OsString> = backend
        .list_items()?
        .into_iter()
        .map(|item| item.id)
        .collect();

    let staged_src = match trash_delete_via_staged_rename(&src, &src_snapshot, backend) {
        Ok(staged) => staged,
        Err(err) => {
            let _ = undo_delete_path(&backup);
            return Err(err);
        }
    };

    let trashed_item = backend.list_items().ok().and_then(|after| {
        after
            .into_iter()
            .find(|item| !before.contains(&item.id) && item.original_path() == staged_src)
    });

    match trashed_item {
        Some(item) => {
            if let Err(err) = backend.rewrite_original_path(&item, &src) {
                warn!(
                    "Failed to rewrite trash info for {}: {}",
                    src.display(),
                    err
                );
            }
            // Remove the backup once we know the trash location.
            let _ = undo_delete_path(&backup);
            Ok(Action::Move {
                from: src,
                to: trash_item_path(&item),
            })
        }
        None => Ok(Action::Delete { path: src, backup }),
    }
}

fn move_single_to_trash(
    path: &str,
    app: &tauri::AppHandle,
    emit_event: bool,
) -> Result<Action, String> {
    let backend = SystemTrashBackend;
    let action = move_single_to_trash_with_backend(path, &backend)?;
    if emit_event {
        let _ = runtime_lifecycle::emit_if_running(app, "trash-changed", ());
    }
    Ok(action)
}

#[tauri::command]
pub fn restore_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> Result<(), String> {
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err("Nothing to restore".into());
    }
    restore_all(selected)
        .map_err(|e| format!("Failed to restore: {e}"))
        .map(|_| {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        })
}

#[tauri::command]
pub fn purge_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> Result<(), String> {
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err("Nothing to delete".into());
    }
    purge_all(selected)
        .map_err(|e| format!("Failed to delete permanently: {e}"))
        .map(|_| {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        })
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::{
        apply_original_trash_fields, cleanup_stale_trash_staging_at,
        decode_percent_encoded_unix_path, encode_trash_info_path,
        load_trash_stage_journal_entries_at, move_single_to_trash_with_backend,
        move_to_trash_many_with_backend, store_trash_stage_journal_entries_at, TrashBackend,
        TrashStageJournalEntry,
    };
    use crate::{
        entry::build_entry,
        icons::icon_ids::PDF_FILE,
        undo::{Action, UndoState},
    };
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::ffi::OsString;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};
    use trash::TrashItem;

    fn uniq_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-fs-test-{label}-{ts}"))
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .expect("open file");
        file.write_all(bytes).expect("write file");
    }

    #[derive(Default)]
    struct FakeTrashBackend {
        items: RefCell<Vec<TrashItem>>,
        list_script: RefCell<VecDeque<Result<Vec<TrashItem>, String>>>,
        delete_calls: RefCell<Vec<PathBuf>>,
        rewrite_calls: RefCell<Vec<(PathBuf, PathBuf)>>,
        fail_delete_call: Cell<Option<usize>>,
        delete_call_count: Cell<usize>,
    }

    impl FakeTrashBackend {
        fn with_fail_on_delete_call(call: usize) -> Self {
            Self {
                fail_delete_call: Cell::new(Some(call)),
                ..Self::default()
            }
        }

        fn queue_list_response(&self, value: Result<Vec<TrashItem>, String>) {
            self.list_script.borrow_mut().push_back(value);
        }
    }

    impl TrashBackend for FakeTrashBackend {
        fn list_items(&self) -> Result<Vec<TrashItem>, String> {
            if let Some(next) = self.list_script.borrow_mut().pop_front() {
                return next;
            }
            Ok(self.items.borrow().clone())
        }

        fn delete_path(&self, path: &Path) -> Result<(), String> {
            let next_call = self.delete_call_count.get().saturating_add(1);
            self.delete_call_count.set(next_call);
            self.delete_calls.borrow_mut().push(path.to_path_buf());

            if self.fail_delete_call.get() == Some(next_call) {
                return Err("simulated trash delete failure".into());
            }

            if let Ok(meta) = fs::symlink_metadata(path) {
                if meta.is_dir() {
                    fs::remove_dir_all(path).map_err(|e| format!("fake delete dir failed: {e}"))?;
                } else {
                    fs::remove_file(path).map_err(|e| format!("fake delete file failed: {e}"))?;
                }
            }

            let name = path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_else(|| OsString::from("item"));
            let original_parent = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"));
            let id = PathBuf::from(format!("/tmp/fake-trash/info/item-{next_call}.trashinfo"))
                .into_os_string();
            self.items.borrow_mut().push(TrashItem {
                id,
                name,
                original_parent,
                time_deleted: 0,
            });
            Ok(())
        }

        fn rewrite_original_path(
            &self,
            item: &TrashItem,
            original_path: &Path,
        ) -> Result<(), String> {
            self.rewrite_calls
                .borrow_mut()
                .push((PathBuf::from(&item.id), original_path.to_path_buf()));
            Ok(())
        }
    }

    #[test]
    fn encode_trash_info_path_percent_encodes_non_unreserved_bytes() {
        let path = PathBuf::from(OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', b'a', b' ', b'b', b'%', 0xFF,
        ]));
        assert_eq!(encode_trash_info_path(&path), "/tmp/a%20b%25%FF");
    }

    #[test]
    fn decode_percent_encoded_unix_path_roundtrips_non_utf8() {
        let original = PathBuf::from(OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', b'x', 0xFF, b' ', b'y',
        ]));
        let encoded = encode_trash_info_path(&original);
        let decoded = decode_percent_encoded_unix_path(&encoded).expect("decode should succeed");
        assert_eq!(decoded, original);
    }

    #[test]
    fn move_single_to_trash_uses_backend_and_rewrites_original_path() {
        let dir = uniq_path("single-trash-success");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("file.txt");
        write_file(&src, b"hello");

        let backend = FakeTrashBackend::default();
        let action =
            move_single_to_trash_with_backend(&src.to_string_lossy(), &backend).expect("success");

        match action {
            Action::Move { from, to: _ } => assert_eq!(from, src),
            other => panic!("expected move action, got {other:?}"),
        }
        assert_eq!(
            backend.delete_calls.borrow().len(),
            1,
            "one delete expected"
        );
        assert_eq!(
            backend.rewrite_calls.borrow().len(),
            1,
            "one rewrite expected"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_single_to_trash_falls_back_to_delete_when_item_not_detected() {
        let dir = uniq_path("single-trash-delete-fallback");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("file.txt");
        write_file(&src, b"hello");

        let backend = FakeTrashBackend::default();
        backend.queue_list_response(Ok(Vec::new()));
        backend.queue_list_response(Ok(Vec::new()));

        let action =
            move_single_to_trash_with_backend(&src.to_string_lossy(), &backend).expect("success");
        match action {
            Action::Delete { path, backup: _ } => assert_eq!(path, src),
            other => panic!("expected delete action, got {other:?}"),
        }
        assert_eq!(
            backend.rewrite_calls.borrow().len(),
            0,
            "no rewrite expected"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_to_trash_many_rolls_back_previous_on_later_failure() {
        let dir = uniq_path("many-trash-rollback");
        let _ = fs::create_dir_all(&dir);
        let src1 = dir.join("a.txt");
        let src2 = dir.join("b.txt");
        write_file(&src1, b"a");
        write_file(&src2, b"b");

        let backend = FakeTrashBackend::with_fail_on_delete_call(2);
        let undo = UndoState::default();
        let result = move_to_trash_many_with_backend(
            vec![
                src1.to_string_lossy().into_owned(),
                src2.to_string_lossy().into_owned(),
            ],
            undo,
            None,
            &backend,
            |_| false,
            |_done, _total, _finished| {},
            || {},
        );

        assert!(result.is_err(), "second delete should fail");
        assert!(
            src1.exists(),
            "first file should be restored after rollback"
        );
        assert!(
            src2.exists(),
            "second file should be rolled back by staging logic"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_stale_trash_staging_recovers_staged_item_and_clears_journal() {
        let dir = uniq_path("cleanup-staged-trash");
        let journal = dir.join("journal.tsv");
        let _ = fs::create_dir_all(&dir);
        let staged = dir.join("browsey-trash-stage-test");
        let original = dir.join("original.txt");
        write_file(&staged, b"staged");

        let entries = vec![TrashStageJournalEntry {
            staged: staged.clone(),
            original: original.clone(),
        }];
        store_trash_stage_journal_entries_at(&journal, &entries).expect("store journal");

        cleanup_stale_trash_staging_at(&journal);

        assert!(!staged.exists(), "staged path should be gone after cleanup");
        assert!(
            original.exists(),
            "original path should be restored after cleanup"
        );
        let remaining = load_trash_stage_journal_entries_at(&journal).expect("load journal");
        assert!(remaining.is_empty(), "journal should be emptied");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn trash_entry_icon_uses_original_path_extension() {
        let dir = uniq_path("trash-original-icon");
        let _ = fs::create_dir_all(&dir);

        let staged = dir.join("browsey-trash-stage-test");
        write_file(&staged, b"dummy");
        let meta = fs::symlink_metadata(&staged).expect("staged metadata");
        let is_link = meta.file_type().is_symlink();
        let mut entry = build_entry(&staged, &meta, is_link, false);

        let item = TrashItem {
            id: OsString::from("/tmp/fake-trash/info/entry.trashinfo"),
            name: OsString::from("report.pdf"),
            original_parent: dir.clone(),
            time_deleted: 0,
        };
        let original_path = item.original_path();
        apply_original_trash_fields(&mut entry, &original_path, &item, &meta, is_link);

        assert_eq!(entry.ext.as_deref(), Some("pdf"));
        assert_eq!(entry.icon_id, PDF_FILE);

        let _ = fs::remove_dir_all(&dir);
    }
}

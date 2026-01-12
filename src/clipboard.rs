use crate::fs_utils::{sanitize_path_follow, unique_path};
use once_cell::sync::Lazy;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Clone, Copy)]
enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone)]
struct ClipboardState {
    entries: Vec<PathBuf>,
    mode: ClipboardMode,
}

static CLIPBOARD: Lazy<Mutex<Option<ClipboardState>>> = Lazy::new(|| Mutex::new(None));

fn ensure_not_child(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.starts_with(src) {
        return Err("Cannot paste a directory into itself".into());
    }
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Failed to create dir {:?}: {e}", dest))?;
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {e}", src))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Refusing to copy symlinks".into());
        }
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            ensure_not_child(&path, &target)?;
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| format!("Failed to copy file {:?}: {e}", path))?;
        }
    }
    Ok(())
}

fn copy_entry(src: &Path, dest: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(src).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Refusing to copy symlinks".into());
    }
    if meta.is_dir() {
        ensure_not_child(src, dest)?;
        copy_dir(src, dest)
    } else {
        fs::copy(src, dest).map_err(|e| format!("Failed to copy file: {e}"))?;
        Ok(())
    }
}

fn delete_entry_path(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

fn move_entry(src: &Path, dest: &Path) -> Result<(), String> {
    ensure_not_child(src, dest)?;
    match fs::rename(src, dest) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_entry(src, dest)?;
            delete_entry_path(src)
        }
    }
}

#[tauri::command]
pub fn set_clipboard_cmd(paths: Vec<String>, mode: String) -> Result<(), String> {
    if paths.is_empty() {
        let mut guard = CLIPBOARD.lock().unwrap();
        *guard = None;
        return Ok(());
    }
    let parsed_mode = match mode.to_lowercase().as_str() {
        "copy" => ClipboardMode::Copy,
        "cut" => ClipboardMode::Cut,
        _ => return Err("Invalid mode".into()),
    };
    let mut entries = Vec::new();
    for p in paths {
        let meta = fs::symlink_metadata(&p).map_err(|e| format!("Path does not exist: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Symlinks are not supported in clipboard".into());
        }
        let clean = sanitize_path_follow(&p, true)?;
        entries.push(clean);
    }
    let mut guard = CLIPBOARD.lock().unwrap();
    *guard = Some(ClipboardState {
        entries,
        mode: parsed_mode,
    });
    Ok(())
}

#[tauri::command]
pub fn paste_clipboard_cmd(dest: String) -> Result<Vec<String>, String> {
    let dest = sanitize_path_follow(&dest, false)?;
    let state = {
        let guard = CLIPBOARD.lock().unwrap();
        guard.clone()
    };
    let Some(state) = state else {
        return Err("Clipboard is empty".into());
    };

    let mut created = Vec::new();
    for src in state.entries.iter() {
        if !src.exists() {
            return Err(format!("Source does not exist: {:?}", src));
        }
        let name = src
            .file_name()
            .ok_or_else(|| "Invalid source path".to_string())?;
        let target_base = dest.join(name);
        let target = unique_path(&target_base);

        match state.mode {
            ClipboardMode::Copy => copy_entry(src, &target)?,
            ClipboardMode::Cut => move_entry(src, &target)?,
        }
        created.push(target.to_string_lossy().to_string());
    }

    if let ClipboardMode::Cut = state.mode {
        let mut guard = CLIPBOARD.lock().unwrap();
        *guard = None;
    }

    Ok(created)
}

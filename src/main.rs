#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icons;
mod search;
mod watcher;
mod statusbar;

use serde::Serialize;
use search::{build_entry, search_recursive, FsEntry};
use std::{cmp::Ordering, fs, path::PathBuf};
use watcher::WatchState;

fn expand_path(raw: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = raw {
        if p == "~" {
            dirs_next::home_dir()
                .ok_or_else(|| "Fant ikke hjemmekatalog".to_string())
        } else if let Some(stripped) = p.strip_prefix("~/") {
            let home = dirs_next::home_dir()
                .ok_or_else(|| "Fant ikke hjemmekatalog".to_string())?;
            Ok(home.join(stripped))
        } else {
            Ok(PathBuf::from(p))
        }
    } else if let Some(home) = dirs_next::home_dir() {
        Ok(home)
    } else {
        std::env::current_dir().map_err(|e| format!("Kunne ikke lese arbeidskatalog: {e}"))
    }
}

#[derive(Serialize)]
struct DirListing {
    current: String,
    entries: Vec<FsEntry>,
}

#[tauri::command]
fn list_dir(path: Option<String>) -> Result<DirListing, String> {
    let base_path = expand_path(path)?;
    let fallback_home = dirs_next::home_dir();

    let target = if base_path.exists() {
        base_path.clone()
    } else if let Some(home) = &fallback_home {
        home.clone()
    } else {
        return Err(format!("{}: finnes ikke", base_path.display()));
    };

    let target = target.canonicalize().unwrap_or_else(|_| target.clone());

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&target)
        .map_err(|e| format!("{}: {e}", target.display()))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = entry
            .metadata()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        entries.push(build_entry(&path, &meta));
    }

    entries.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") => Ordering::Less,
        ("file", "dir") => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(DirListing {
        current: target.to_string_lossy().into_owned(),
        entries,
    })
}

#[tauri::command]
fn search(path: Option<String>, query: String) -> Result<Vec<FsEntry>, String> {
    let base_path = expand_path(path)?;
    let target = if base_path.exists() {
        base_path
    } else if let Some(home) = dirs_next::home_dir() {
        home
    } else {
        return Err("Fant ikke startkatalog".to_string());
    };
    search_recursive(target, query)
}

#[tauri::command]
fn watch_dir(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let base_path = expand_path(path)?;
    let target = if base_path.exists() {
        base_path
    } else if let Some(home) = dirs_next::home_dir() {
        home
    } else {
        return Err("Fant ikke startkatalog".to_string());
    };
    watcher::start_watch(app, target, &state)
}

fn main() {
    tauri::Builder::default()
        .manage(WatchState::default())
        .invoke_handler(tauri::generate_handler![list_dir, search, watch_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

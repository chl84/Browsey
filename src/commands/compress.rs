use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::Emitter;
use walkdir::WalkDir;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use crate::fs_utils::{check_no_symlink_components, sanitize_path_follow};

const CHUNK: usize = 4 * 1024 * 1024;

fn ensure_same_parent(paths: &[PathBuf]) -> Result<PathBuf, String> {
    let mut parent: Option<PathBuf> = None;
    for p in paths {
        match p.parent() {
            Some(par) => match parent {
                Some(ref prev) if prev != par => {
                    return Err("All items must be in the same folder to compress together".into())
                }
                Some(_) => {}
                None => parent = Some(par.to_path_buf()),
            },
            None => return Err("Cannot compress filesystem root".into()),
        }
    }
    parent.ok_or_else(|| "Missing parent for paths".into())
}

fn safe_name(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Name cannot be empty".into());
    }
    if name.contains(['/', '\\']) {
        return Err("Name cannot contain path separators".into());
    }
    Ok(name.to_string())
}

fn destination_path(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let mut base = safe_name(name)?;
    if !base.to_lowercase().ends_with(".zip") {
        base.push_str(".zip");
    }
    let mut candidate = parent.join(&base);
    let mut idx = 1;
    while candidate.exists() {
        candidate = parent.join(format!("{} ({idx}).zip", base.trim_end_matches(".zip")));
        idx += 1;
    }
    Ok(candidate)
}

fn add_path_to_zip(
    zip: &mut ZipWriter<BufWriter<File>>,
    base: &Path,
    path: &Path,
    level: u32,
    options: FileOptions,
    progress: Option<&ProgressEmitter>,
) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Symlinks are not supported for compression".into());
    }

    let rel = path
        .strip_prefix(base)
        .map_err(|_| "Paths must share the same parent")?;
    let mut rel_name = rel.to_string_lossy().replace('\\', "/");
    let is_dir = meta.is_dir();
    if is_dir && !rel_name.ends_with('/') {
        rel_name.push('/');
    }

    let method = if level == 0 {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let opts = options
        .clone()
        .compression_method(method)
        .compression_level(Some(level as i32));

    if is_dir {
        zip.add_directory(rel_name, opts)
            .map_err(|e| format!("Failed to add directory to zip: {e}"))?;
        return Ok(());
    }

    zip.start_file(rel_name, opts)
        .map_err(|e| format!("Failed to start zip entry: {e}"))?;
    let file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut reader = BufReader::with_capacity(CHUNK, file);
    copy_with_progress(&mut reader, zip, progress)
        .map_err(|e| format!("Failed to write file to zip: {e}"))?;
    Ok(())
}

fn collect_paths(input: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for path in input {
        let meta =
            fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Symlinks are not supported for compression".into());
        }
        if meta.is_dir() {
            for entry in WalkDir::new(path) {
                let entry = entry.map_err(|e| format!("Failed to read directory: {e}"))?;
                let p = entry.into_path();
                let meta = fs::symlink_metadata(&p)
                    .map_err(|e| format!("Failed to read metadata: {e}"))?;
                if meta.file_type().is_symlink() {
                    return Err("Symlinks are not supported for compression".into());
                }
                out.push(p);
            }
        } else {
            out.push(path.clone());
        }
    }
    Ok(out)
}

#[derive(Serialize, Clone, Copy)]
struct CompressProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(Clone)]
struct ProgressEmitter {
    app: tauri::AppHandle,
    event: String,
    total: u64,
    done: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl ProgressEmitter {
    fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn add(&self, delta: u64) {
        let done = self
            .done
            .fetch_add(delta, std::sync::atomic::Ordering::Relaxed)
            .saturating_add(delta);
        let _ = self.app.emit(
            &self.event,
            CompressProgressPayload {
                bytes: done,
                total: self.total,
                finished: false,
            },
        );
    }

    fn finish(&self) {
        let done = self.done.load(std::sync::atomic::Ordering::Relaxed);
        let _ = self.app.emit(
            &self.event,
            CompressProgressPayload {
                bytes: done,
                total: self.total,
                finished: true,
            },
        );
    }
}

#[tauri::command]
pub async fn compress_entries(
    app: tauri::AppHandle,
    paths: Vec<String>,
    name: Option<String>,
    level: Option<u32>,
    progress_event: Option<String>,
) -> Result<String, String> {
    let task =
        tauri::async_runtime::spawn_blocking(move || do_compress(app, paths, name, level, progress_event));
    task.await
        .map_err(|e| format!("Compression task failed: {e}"))?
}

fn do_compress(
    app: tauri::AppHandle,
    paths: Vec<String>,
    name: Option<String>,
    level: Option<u32>,
    progress_event: Option<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("Nothing to compress".into());
    }
    let mut resolved: Vec<PathBuf> = Vec::new();
    for raw in paths {
        let pb = sanitize_path_follow(&raw, true)?;
        check_no_symlink_components(&pb)?;
        resolved.push(pb);
    }
    let parent = ensure_same_parent(&resolved)?;

    let suggested = if resolved.len() == 1 {
        resolved
            .get(0)
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("archive")
            .to_string()
    } else {
        "Archive".to_string()
    };
    let dest_name = name.unwrap_or(suggested);
    let lvl = level.unwrap_or(6).min(9);
    let mut dest = destination_path(&parent, &dest_name)?;
    let file = loop {
        match File::options().write(true).create_new(true).open(&dest) {
            Ok(f) => break f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                dest = destination_path(&parent, &dest_name)?;
                continue;
            }
            Err(e) => return Err(format!("Failed to create destination: {e}")),
        }
    };

    let mut writer = ZipWriter::new(BufWriter::new(file));
    let opts = FileOptions::default();

    let all = collect_paths(&resolved)?;
    let progress = progress_event.map(|evt| ProgressEmitter::new(app, evt, total_size_bytes(&all)));
    for path in all {
        add_path_to_zip(&mut writer, &parent, &path, lvl, opts.clone(), progress.as_ref())?;
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;

    if let Some(p) = progress.as_ref() {
        p.finish();
    }

    Ok(dest.to_string_lossy().into_owned())
}

fn total_size_bytes(paths: &[PathBuf]) -> u64 {
    let mut total = 0u64;
    for p in paths {
        if let Ok(meta) = fs::symlink_metadata(p) {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    mut progress: Option<&ProgressEmitter>,
) -> io::Result<u64> {
    let mut buf = vec![0u8; CHUNK];
    let mut written = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
        if let Some(p) = progress.as_mut() {
            p.add(n as u64);
        }
    }
    Ok(written)
}

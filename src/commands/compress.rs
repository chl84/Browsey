use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
    env,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use chrono::{DateTime as ChronoDateTime, Datelike, Local, Timelike};
use serde::Serialize;
use tauri::Emitter;
use walkdir::WalkDir;
use zip::{write::FileOptions, CompressionMethod, DateTime as ZipDateTime, ZipWriter};

use crate::fs_utils::sanitize_path_nofollow;

const CHUNK: usize = 4 * 1024 * 1024;
const FILE_READ_BUF: usize = 256 * 1024;
const PROGRESS_EMIT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
struct EntryMeta {
    path: PathBuf,
    rel_path: PathBuf,
    is_dir: bool,
    is_symlink: bool,
    mode: Option<u32>,
    modified: Option<ZipDateTime>,
    symlink_target: Option<PathBuf>,
}

#[cfg(unix)]
fn metadata_mode(meta: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.mode())
}

#[cfg(not(unix))]
fn metadata_mode(_meta: &fs::Metadata) -> Option<u32> {
    None
}

fn system_time_to_zip_datetime(time: SystemTime) -> Option<ZipDateTime> {
    let dt: ChronoDateTime<Local> = time.into();
    ZipDateTime::from_date_and_time(
        dt.year() as u16,
        dt.month() as u8,
        dt.day() as u8,
        dt.hour() as u8,
        dt.minute() as u8,
        dt.second() as u8,
    )
    .ok()
}

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

fn resolve_input_path(raw: &str) -> Result<PathBuf, String> {
    let pb = sanitize_path_nofollow(raw, true)?;
    let abs = if pb.is_absolute() {
        pb
    } else {
        env::current_dir()
            .map_err(|e| format!("Failed to resolve current directory: {e}"))?
            .join(pb)
    };
    Ok(abs)
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

fn destination_path(parent: &Path, name: &str, idx: usize) -> Result<PathBuf, String> {
    let mut base = safe_name(name)?;
    if !base.to_lowercase().ends_with(".zip") {
        base.push_str(".zip");
    }
    let stem = base.trim_end_matches(".zip");
    let suffix = if idx == 0 {
        String::new()
    } else {
        format!(" ({idx})")
    };
    Ok(parent.join(format!("{stem}{suffix}.zip")))
}

fn add_path_to_zip(
    zip: &mut ZipWriter<BufWriter<File>>,
    entry: &EntryMeta,
    deflated_opts: &FileOptions,
    stored_opts: &FileOptions,
    progress: Option<&ProgressEmitter>,
    buf: &mut [u8],
) -> Result<(), String> {
    let mut rel_name = entry.rel_path.to_string_lossy().replace('\\', "/");
    if entry.is_dir && !rel_name.ends_with('/') {
        rel_name.push('/');
    }

    let base_opts = if entry.is_dir || !is_precompressed(&entry.path) {
        *deflated_opts
    } else {
        *stored_opts
    };
    let opts = with_entry_metadata(base_opts, entry);

    if entry.is_dir {
        zip.add_directory(rel_name, opts)
            .map_err(|e| format!("Failed to add directory to zip: {e}"))?;
        return Ok(());
    }
    zip.start_file(rel_name, opts)
        .map_err(|e| format!("Failed to start zip entry: {e}"))?;
    let file = File::open(&entry.path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut reader = BufReader::with_capacity(FILE_READ_BUF, file);
    copy_with_progress(&mut reader, zip, progress, buf)
        .map_err(|e| format!("Failed to write file to zip: {e}"))?;
    Ok(())
}

fn with_entry_metadata(base: FileOptions, entry: &EntryMeta) -> FileOptions {
    let mut opts = base;
    if let Some(mode) = entry.mode {
        opts = opts.unix_permissions(mode);
    }
    if let Some(modified) = entry.modified {
        opts = opts.last_modified_time(modified);
    }
    opts
}

fn is_precompressed(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext)
            if matches!(
                ext.as_str(),
                "zip"
                    | "gz"
                    | "tgz"
                    | "bz2"
                    | "tbz"
                    | "xz"
                    | "lz"
                    | "lz4"
                    | "zst"
                    | "7z"
                    | "rar"
                    | "jpg"
                    | "jpeg"
                    | "png"
                    | "gif"
                    | "mp3"
                    | "mp4"
                    | "m4a"
                    | "mkv"
                    | "mov"
                    | "avi"
                    | "webm"
                    | "pdf"
            ) =>
        {
            true
        }
        _ => false,
    }
}

fn collect_entries(base: &Path, input: &[PathBuf]) -> Result<(Vec<EntryMeta>, u64), String> {
    let mut out = Vec::new();
    let mut total_size = 0u64;

    let mut push_entry = |p: PathBuf, meta: fs::Metadata| -> Result<(), String> {
        let rel = p
            .strip_prefix(base)
            .map_err(|_| "Paths must share the same parent")?
            .to_path_buf();
        let file_type = meta.file_type();
        let is_dir = file_type.is_dir();
        let is_symlink = file_type.is_symlink();
        if file_type.is_file() {
            total_size = total_size.saturating_add(meta.len());
        }

        let modified = meta.modified().ok().and_then(system_time_to_zip_datetime);
        let mode = metadata_mode(&meta);
        let symlink_target = if is_symlink {
            Some(
                fs::read_link(&p)
                    .map_err(|e| format!("Failed to read symlink target for {}: {e}", p.display()))?,
            )
        } else {
            None
        };

        out.push(EntryMeta {
            path: p,
            rel_path: rel,
            is_dir,
            is_symlink,
            mode,
            modified,
            symlink_target,
        });
        Ok(())
    };

    for path in input {
        let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.is_dir() {
            for entry in WalkDir::new(path).follow_links(false) {
                let entry = entry.map_err(|e| format!("Failed to read directory: {e}"))?;
                let p = entry.into_path();
                let meta = fs::symlink_metadata(&p)
                    .map_err(|e| format!("Failed to read metadata: {e}"))?;
                push_entry(p, meta)?;
            }
        } else {
            push_entry(path.clone(), meta)?;
        }
    }

    Ok((out, total_size))
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
    done: Arc<AtomicU64>,
    last_emit: Arc<AtomicU64>,
}

impl ProgressEmitter {
    fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: Arc::new(AtomicU64::new(0)),
            last_emit: Arc::new(AtomicU64::new(0)),
        }
    }

    fn add(&self, delta: u64) {
        let done = self.done.fetch_add(delta, Ordering::Relaxed).saturating_add(delta);
        let last = self.last_emit.load(Ordering::Relaxed);
        if done.saturating_sub(last) >= PROGRESS_EMIT_BYTES {
            if self
                .last_emit
                .compare_exchange(last, done, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let _ = self.app.emit(
                    &self.event,
                    CompressProgressPayload {
                        bytes: done,
                        total: self.total,
                        finished: false,
                    },
                );
            }
        }
    }

    fn finish(&self) {
        let done = self.done.load(Ordering::Relaxed);
        self.last_emit.store(done, Ordering::Relaxed);
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
        let pb = resolve_input_path(&raw)?;
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
    let mut dest_idx = 0usize;
    let (dest, file) = loop {
        let candidate = destination_path(&parent, &dest_name, dest_idx)?;
        match File::options().write(true).create_new(true).open(&candidate) {
            Ok(f) => break (candidate, f),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                dest_idx = dest_idx.saturating_add(1);
                continue;
            }
            Err(e) => return Err(format!("Failed to create destination: {e}")),
        }
    };

    let mut writer = ZipWriter::new(BufWriter::with_capacity(CHUNK, file));

    let (entries, total_size) = collect_entries(&parent, &resolved)?;
    let progress = progress_event.map(|evt| ProgressEmitter::new(app, evt, total_size));
    let mut buf = vec![0u8; CHUNK];

    let method = if lvl == 0 {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let deflated_opts = FileOptions::default()
        .compression_method(method)
        .compression_level(Some(lvl as i32));

    let stored_opts = FileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .compression_level(Some(0));

    for entry in &entries {
        if entry.is_symlink {
            let mut rel_name = entry.rel_path.to_string_lossy().replace('\\', "/");
            if rel_name.ends_with('/') {
                rel_name.pop();
            }
            let target = entry
                .symlink_target
                .as_ref()
                .ok_or_else(|| "Missing symlink target".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let opts = with_entry_metadata(stored_opts, entry);
            writer
                .add_symlink(rel_name, target, opts)
                .map_err(|e| format!("Failed to add symlink to zip: {e}"))?;
        } else {
            add_path_to_zip(
                &mut writer,
                entry,
                &deflated_opts,
                &stored_opts,
                progress.as_ref(),
                &mut buf,
            )?;
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;

    if let Some(p) = progress.as_ref() {
        p.finish();
    }

    Ok(dest.to_string_lossy().into_owned())
}

fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    mut progress: Option<&ProgressEmitter>,
    buf: &mut [u8],
) -> io::Result<u64> {
    let mut written = 0u64;
    loop {
        let n = reader.read(buf)?;
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

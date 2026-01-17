use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};

use bzip2::read::BzDecoder;
use flate2::read::{GzDecoder, MultiGzDecoder};
use rayon::prelude::*;
use serde::Serialize;
use tar::Archive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

use crate::fs_utils::{
    check_no_symlink_components, debug_log, sanitize_path_follow, sanitize_path_nofollow, unique_path,
};
use tauri::Emitter;

const CHUNK: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    Gz,
    Bz2,
    Xz,
    Zstd,
}

#[derive(Default, Clone)]
struct SkipStats {
    symlinks: Arc<AtomicUsize>,
    unsupported: Arc<AtomicUsize>,
}

impl SkipStats {
    fn skip_symlink(&self, path: &str) {
        self.symlinks.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping symlink entry while extracting: {path}"));
    }

    fn skip_unsupported(&self, path: &str, reason: &str) {
        self.unsupported.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping unsupported entry {path}: {reason}"));
    }
}

#[derive(Serialize)]
pub struct ExtractResult {
    pub destination: String,
    pub skipped_symlinks: usize,
    pub skipped_entries: usize,
}

#[derive(Serialize, Clone, Copy)]
struct ExtractProgressPayload {
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
}

impl ProgressEmitter {
    fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: Arc::new(AtomicU64::new(0)),
        }
    }

    fn add(&self, delta: u64) {
        let done = self.done.fetch_add(delta, Ordering::Relaxed).saturating_add(delta);
        let _ = self.app.emit(
            &self.event,
            ExtractProgressPayload {
                bytes: done,
                total: self.total,
                finished: false,
            },
        );
    }

    fn finish(&self) {
        let done = self.done.load(Ordering::Relaxed);
        let _ = self.app.emit(
            &self.event,
            ExtractProgressPayload {
                bytes: done,
                total: self.total,
                finished: true,
            },
        );
    }
}

#[tauri::command]
pub async fn extract_archive(
    app: tauri::AppHandle,
    path: String,
    progress_event: Option<String>,
) -> Result<ExtractResult, String> {
    let task = tauri::async_runtime::spawn_blocking(move || do_extract(app, path, progress_event));
    task.await
        .map_err(|e| format!("Extraction task failed: {e}"))?
}

fn do_extract(
    app: tauri::AppHandle,
    path: String,
    progress_event: Option<String>,
) -> Result<ExtractResult, String> {
    let nofollow = sanitize_path_nofollow(&path, true)?;
    let meta = fs::symlink_metadata(&nofollow)
        .map_err(|e| format!("Failed to read archive metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Symlink archives are not supported".into());
    }

    let archive_path = sanitize_path_follow(&path, true)?;
    check_no_symlink_components(&archive_path)?;

    if !archive_path.is_file() {
        return Err("Only files can be extracted".into());
    }

    let parent = archive_path
        .parent()
        .ok_or_else(|| "Cannot extract archive at filesystem root".to_string())?;

    let total_hint = meta.len();
    let progress = progress_event.map(|evt| ProgressEmitter::new(app, evt, total_hint));

    let kind = detect_archive(&archive_path)?;
    let stats = SkipStats::default();
    let destination = match kind {
        ArchiveKind::Zip => {
            let dest_dir = prepare_output_dir(parent, &archive_path)?;
            extract_zip(&archive_path, &dest_dir, &stats, progress.as_ref())?;
            dest_dir
        }
        ArchiveKind::Tar => {
            let dest_dir = prepare_output_dir(parent, &archive_path)?;
            extract_tar(
                File::open(&archive_path).map_err(map_io("open tar"))?,
                &dest_dir,
                &stats,
                progress.as_ref(),
            )?;
            dest_dir
        }
        ArchiveKind::TarGz => {
            let dest_dir = prepare_output_dir(parent, &archive_path)?;
            let reader = GzDecoder::new(File::open(&archive_path).map_err(map_io("open tar.gz"))?);
            extract_tar(reader, &dest_dir, &stats, progress.as_ref())?;
            dest_dir
        }
        ArchiveKind::TarBz2 => {
            let dest_dir = prepare_output_dir(parent, &archive_path)?;
            let reader = BzDecoder::new(File::open(&archive_path).map_err(map_io("open tar.bz2"))?);
            extract_tar(reader, &dest_dir, &stats, progress.as_ref())?;
            dest_dir
        }
        ArchiveKind::TarXz => {
            let dest_dir = prepare_output_dir(parent, &archive_path)?;
            let reader = XzDecoder::new(File::open(&archive_path).map_err(map_io("open tar.xz"))?);
            extract_tar(reader, &dest_dir, &stats, progress.as_ref())?;
            dest_dir
        }
        ArchiveKind::TarZstd => {
            let dest_dir = prepare_output_dir(parent, &archive_path)?;
            let reader =
                ZstdDecoder::new(File::open(&archive_path).map_err(map_io("open tar.zst"))?).map_err(|e| {
                    format!("Failed to create zstd decoder: {e}")
                })?;
            extract_tar(reader, &dest_dir, &stats, progress.as_ref())?;
            dest_dir
        }
        ArchiveKind::Gz => {
            let reader = MultiGzDecoder::new(BufReader::with_capacity(
                CHUNK,
                File::open(&archive_path).map_err(map_io("open gz"))?,
            ));
            let dest = decompress_single(reader, &archive_path, parent, progress.as_ref())?;
            dest
        }
        ArchiveKind::Bz2 => {
            let reader = BzDecoder::new(BufReader::with_capacity(
                CHUNK,
                File::open(&archive_path).map_err(map_io("open bz2"))?,
            ));
            decompress_single(reader, &archive_path, parent, progress.as_ref())?
        }
        ArchiveKind::Xz => {
            let reader = XzDecoder::new(BufReader::with_capacity(
                CHUNK,
                File::open(&archive_path).map_err(map_io("open xz"))?,
            ));
            decompress_single(reader, &archive_path, parent, progress.as_ref())?
        }
        ArchiveKind::Zstd => {
            let reader = ZstdDecoder::new(BufReader::with_capacity(
                CHUNK,
                File::open(&archive_path).map_err(map_io("open zst"))?,
            ))
            .map_err(|e| format!("Failed to create zstd decoder: {e}"))?;
            decompress_single(reader, &archive_path, parent, progress.as_ref())?
        }
    };

    if let Some(p) = progress.as_ref() {
        p.finish();
    }

    Ok(ExtractResult {
        destination: destination.to_string_lossy().into_owned(),
        skipped_symlinks: stats.symlinks.load(Ordering::Relaxed),
        skipped_entries: stats.unsupported.load(Ordering::Relaxed),
    })
}

fn map_io(action: &'static str) -> impl FnOnce(io::Error) -> String {
    move |e| format!("Failed to {action}: {e}")
}

fn detect_archive(path: &Path) -> Result<ArchiveKind, String> {
    let mut f = File::open(path).map_err(map_io("open archive for detection"))?;
    let mut buf = [0u8; 512];
    let n = f.read(&mut buf).map_err(map_io("read archive header"))?;
    let magic = &buf[..n];

    if magic.starts_with(b"PK\x03\x04") || magic.starts_with(b"PK\x05\x06") || magic.starts_with(b"PK\x07\x08") {
        return Ok(ArchiveKind::Zip);
    }
    if magic.starts_with(&[0x1F, 0x8B]) {
        return Ok(if looks_like_tar(path) {
            ArchiveKind::TarGz
        } else {
            ArchiveKind::Gz
        });
    }
    if magic.starts_with(b"BZh") {
        return Ok(if looks_like_tar(path) {
            ArchiveKind::TarBz2
        } else {
            ArchiveKind::Bz2
        });
    }
    if magic.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        return Ok(if looks_like_tar(path) {
            ArchiveKind::TarXz
        } else {
            ArchiveKind::Xz
        });
    }
    if magic.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return Ok(if looks_like_tar(path) {
            ArchiveKind::TarZstd
        } else {
            ArchiveKind::Zstd
        });
    }
    if magic.len() >= 262 && &magic[257..262] == b"ustar" {
        return Ok(ArchiveKind::Tar);
    }

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let guess = if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveKind::TarGz)
    } else if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
        Some(ArchiveKind::TarBz2)
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        Some(ArchiveKind::TarXz)
    } else if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        Some(ArchiveKind::TarZstd)
    } else if name.ends_with(".tar") {
        Some(ArchiveKind::Tar)
    } else if name.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if name.ends_with(".gz") {
        Some(ArchiveKind::Gz)
    } else if name.ends_with(".bz2") {
        Some(ArchiveKind::Bz2)
    } else if name.ends_with(".xz") {
        Some(ArchiveKind::Xz)
    } else if name.ends_with(".zst") {
        Some(ArchiveKind::Zstd)
    } else {
        None
    };

    guess.ok_or_else(|| "Unsupported archive format".to_string())
}

fn looks_like_tar(path: &Path) -> bool {
    let lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();
    lower.contains(".tar")
}

fn prepare_output_dir(parent: &Path, _archive: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(parent).map_err(map_io("ensure parent dir"))?;
    Ok(parent.to_path_buf())
}

fn strip_known_suffixes(name: &str) -> String {
    let lower = name.to_lowercase();
    for suffix in [
        ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst", ".tar",
    ] {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    if let Some((stem, _ext)) = name.rsplit_once('.') {
        if !stem.is_empty() {
            return stem.to_string();
        }
    }
    name.to_string()
}

fn extract_zip(
    path: &Path,
    dest_dir: &Path,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
) -> Result<(), String> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut work = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        let raw_name = entry.name().to_string();
        let enclosed = entry.enclosed_name().ok_or_else(|| {
            stats.skip_unsupported(&raw_name, "path traversal");
            "skipped".to_string()
        });
        let enclosed = match enclosed {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let clean_rel = match clean_relative_path(&enclosed) {
            Ok(p) => p,
            Err(err) => {
                stats.skip_unsupported(&raw_name, &err);
                continue;
            }
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }
        let is_symlink = entry
            .unix_mode()
            .map(|mode| (mode & 0o170000) == 0o120000)
            .unwrap_or(false);
        if is_symlink {
            stats.skip_symlink(&raw_name);
            continue;
        }
        let dest_path = dest_dir.join(clean_rel);
        if let Some(parent) = dest_path.parent() {
            if parent.exists() && !parent.is_dir() {
                stats.skip_unsupported(&raw_name, "parent exists as a file");
                continue;
            }
        }
        if entry.is_dir() || raw_name.ends_with('/') {
            if dest_path.exists() && dest_path.is_file() {
                stats.skip_unsupported(&raw_name, "destination exists as a file");
                continue;
            }
            fs::create_dir_all(&dest_path).map_err(map_io("create zip directory"))?;
            continue;
        }
        let final_path = finalize_file_path(dest_path);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).map_err(map_io("create zip parent"))?;
        }
        let size_hint = entry.size();
        work.push((i, final_path, raw_name, size_hint));
    }
    drop(archive);

    let arc_path = path.to_path_buf();
    work.par_iter().try_for_each(|(idx, dest_path, raw_name, _size)| -> Result<(), String> {
        let mut archive = ZipArchive::new(File::open(&arc_path).map_err(map_io("open zip chunk"))?)
            .map_err(|e| format!("Failed to read zip: {e}"))?;
        let mut entry = archive
            .by_index(*idx)
            .map_err(|e| format!("Failed to read zip entry {}: {e}", idx))?;
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(dest_path)
            .map_err(map_io("create zip file"))?;
        let mut out = BufWriter::with_capacity(CHUNK, file);
        if let Err(e) = copy_with_progress(&mut entry, &mut out, progress) {
            let msg = format!("Failed to write zip entry {raw_name}: {e}");
            return Err(msg);
        }
        Ok(())
    })?;

    Ok(())
}

fn extract_tar<R: Read>(
    reader: R,
    dest_dir: &Path,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
) -> Result<(), String> {
    let mut archive = Archive::new(reader);
    for entry_result in archive.entries().map_err(|e| format!("Failed to iterate tar: {e}"))? {
        let mut entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        let entry_type = entry.header().entry_type();
        let raw_path = entry
            .path()
            .map_err(|e| format!("Invalid tar entry path: {e}"))?
            .into_owned();
        let raw_str = raw_path.to_string_lossy().to_string();

        if entry_type.is_symlink() || entry_type.is_hard_link() {
            stats.skip_symlink(&raw_str);
            continue;
        }
        if !(entry_type.is_dir() || entry_type.is_file()) {
            stats.skip_unsupported(&raw_str, "unsupported type");
            continue;
        }
        let clean_rel = match clean_relative_path(&raw_path) {
            Ok(p) => p,
            Err(err) => {
                stats.skip_unsupported(&raw_str, &err);
                continue;
            }
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }

        let dest_path = dest_dir.join(clean_rel);
        if let Some(parent) = dest_path.parent() {
            if parent.exists() && !parent.is_dir() {
                stats.skip_unsupported(&raw_str, "parent exists as a file");
                continue;
            }
        }
        if entry_type.is_dir() {
            if dest_path.exists() && dest_path.is_file() {
                stats.skip_unsupported(&raw_str, "destination exists as a file");
                continue;
            }
            fs::create_dir_all(&dest_path).map_err(map_io("create tar dir"))?;
            continue;
        }

        let final_path = finalize_file_path(dest_path);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).map_err(map_io("create tar parent"))?;
        }
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(&final_path)
            .map_err(map_io("create tar file"))?;
        let mut out = BufWriter::with_capacity(CHUNK, file);
        copy_with_progress(&mut entry, &mut out, progress).map_err(map_io("write tar entry"))?;
    }
    Ok(())
}

fn decompress_single<R: Read>(
    mut reader: R,
    archive: &Path,
    parent: &Path,
    progress: Option<&ProgressEmitter>,
) -> Result<PathBuf, String> {
    let mut dest_name = archive
        .file_name()
        .and_then(|s| s.to_str())
        .map(strip_known_suffixes)
        .unwrap_or_else(|| "extracted".to_string());
    if dest_name.is_empty() {
        dest_name = "extracted".to_string();
    }
    let dest_path = finalize_file_path(parent.join(dest_name));
    if let Some(parent_dir) = dest_path.parent() {
        fs::create_dir_all(parent_dir).map_err(map_io("create output dir"))?;
    }
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(&dest_path)
        .map_err(map_io("create output file"))?;
    let mut out = BufWriter::with_capacity(CHUNK, file);
    copy_with_progress(&mut reader, &mut out, progress).map_err(map_io("write decompressed file"))?;
    Ok(dest_path)
}

fn finalize_file_path(dest_path: PathBuf) -> PathBuf {
    if dest_path.exists() {
        unique_path(&dest_path)
    } else {
        dest_path
    }
}

fn clean_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(p) => cleaned.push(p),
            std::path::Component::CurDir => {}
            _ => return Err("Refusing path with traversal or absolute components".into()),
        }
    }
    Ok(cleaned)
}

fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    progress: Option<&ProgressEmitter>,
) -> io::Result<u64> {
    let mut buf = vec![0u8; CHUNK];
    let mut written: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
        if let Some(p) = progress {
            p.add(n as u64);
        }
    }
    Ok(written)
}

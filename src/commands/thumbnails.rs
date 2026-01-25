use std::fs;
use std::path::{Path, PathBuf};

use blake3::Hasher;
use image::ImageReader;
use image::{imageops::FilterType, GenericImageView, ImageFormat};
use once_cell::sync::Lazy;
use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::oneshot;

use crate::fs_utils::debug_log;

const MAX_DIM_DEFAULT: u32 = 96;
const MAX_DIM_HARD_LIMIT: u32 = 512;
const MIN_DIM_HARD_LIMIT: u32 = 32;
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const POOL_MAX_THREADS: usize = 4;
const CACHE_MAX_BYTES: u64 = 300 * 1024 * 1024;
const CACHE_MAX_FILES: usize = 2000;
const MAX_SOURCE_DIM: u32 = 20000;
const DECODE_TIMEOUT_MS: u64 = 400;
const GLOBAL_MAX_INFLIGHT: usize = 16;

static THUMB_POOL: Lazy<ThreadPool> = Lazy::new(|| {
    let threads = std::cmp::max(2, std::cmp::min(num_cpus::get(), POOL_MAX_THREADS));
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("thumb-{}", i))
        .build()
        .expect("failed to build thumbnail pool")
});

static INFLIGHT: Lazy<std::sync::Mutex<HashMap<String, Vec<oneshot::Sender<Result<ThumbnailResponse, String>>>>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
static GLOBAL_ACTIVE: Lazy<std::sync::Mutex<usize>> = Lazy::new(|| std::sync::Mutex::new(0usize));

#[derive(Serialize, Clone)]
pub struct ThumbnailResponse {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub cached: bool,
}

#[tauri::command]
pub async fn get_thumbnail(path: String, max_dim: Option<u32>) -> Result<ThumbnailResponse, String> {
    let max_dim = max_dim
        .unwrap_or(MAX_DIM_DEFAULT)
        .clamp(MIN_DIM_HARD_LIMIT, MAX_DIM_HARD_LIMIT);

    let target = sanitize_input_path(&path)?;
    // Quick permission check (fail fast on unreadable files)
    if let Err(e) = fs::File::open(&target) {
        return Err(format!("Cannot read file: {e}"));
    }
    let meta = fs::metadata(&target).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if !meta.is_file() {
        return Err("Target is not a file".to_string());
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "File too large for thumbnail (>{} MB)",
            MAX_FILE_BYTES / 1024 / 1024
        ));
    }
    let mtime = meta.modified().ok();

    let cache_dir = cache_dir()?;
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create thumbnail cache dir: {e}"))?;

    let key = cache_key(&target, mtime, max_dim);
    let cache_path = cache_dir.join(format!("{key}.png"));

    if let Some((w, h)) = cached_dims(&cache_path) {
        return Ok(ThumbnailResponse {
            path: cache_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
            cached: true,
        });
    }

    // In-flight deduplication
    if let Some(rx) = register_or_wait(&key) {
        let res: Result<Result<ThumbnailResponse, String>, _> = rx.await;
        return res
            .map_err(|_| "Thumbnail task cancelled".to_string())?
            .map(|mut r| {
                r.cached = true;
                r
            });
    }

    let task_path = target.clone();
    let task_cache = cache_path.clone();

    if !try_inc_global_active() {
        notify_waiters(&key, Err("Too many concurrent thumbnails".into()));
        return Err("Too many concurrent thumbnails".into());
    }

    let res = tauri::async_runtime::spawn_blocking(move || {
        THUMB_POOL.install(|| generate_thumbnail(&task_path, &task_cache, max_dim))
    })
    .await
    .map_err(|e| format!("Thumbnail task cancelled: {e}"));

    dec_global_active();

    let res = res?;

    let res = match res {
        Ok(r) => {
            notify_waiters(&key, Ok(r.clone()));
            trim_cache(&cache_dir, CACHE_MAX_BYTES, CACHE_MAX_FILES);
            Ok(r)
        }
        Err(err) => {
            notify_waiters(&key, Err(err.clone()));
            Err(err)
        }
    };

    res
}

fn try_inc_global_active() -> bool {
    let mut g = GLOBAL_ACTIVE.lock().expect("global active poisoned");
    if *g >= GLOBAL_MAX_INFLIGHT {
        return false;
    }
    *g += 1;
    true
}

fn dec_global_active() {
    let mut g = GLOBAL_ACTIVE.lock().expect("global active poisoned");
    if *g > 0 {
        *g -= 1;
    }
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = dirs_next::cache_dir()
        .or_else(dirs_next::data_dir)
        .unwrap_or_else(|| std::env::temp_dir());
    Ok(base.join("browsey").join("thumbs"))
}

fn cache_key(path: &Path, mtime: Option<std::time::SystemTime>, max_dim: u32) -> String {
    let mut hasher = Hasher::new();
    hasher.update(path.to_string_lossy().as_bytes());
    if let Some(ts) = mtime.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
        hasher.update(&ts.as_secs().to_le_bytes());
        hasher.update(&ts.subsec_nanos().to_le_bytes());
    }
    hasher.update(&max_dim.to_le_bytes());
    let hash = hasher.finalize();
    hash.to_hex().to_string()
}

fn cached_dims(path: &Path) -> Option<(u32, u32)> {
    if !path.exists() {
        return None;
    }
    ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn generate_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
) -> Result<ThumbnailResponse, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("Open failed: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess format: {e}"))?;

    // format allowlist (image crate supported set)
    let fmt = reader.format().ok_or("Unsupported image format")?;
    match fmt {
        ImageFormat::Png
        | ImageFormat::Jpeg
        | ImageFormat::Gif
        | ImageFormat::Bmp
        | ImageFormat::Ico
        | ImageFormat::Pnm
        | ImageFormat::Tiff
        | ImageFormat::Tga
        | ImageFormat::WebP
        | ImageFormat::Hdr
        | ImageFormat::Dds => {}
        _ => return Err("Unsupported image format".into()),
    }

    let img = decode_with_timeout(reader, Duration::from_millis(DECODE_TIMEOUT_MS))?;

    let (src_w, src_h) = img.dimensions();
    if src_w > MAX_SOURCE_DIM || src_h > MAX_SOURCE_DIM {
        return Err("Image dimensions too large for thumbnail".into());
    }
    let thumb = img.resize(max_dim, max_dim, FilterType::Lanczos3);
    let (w, h) = thumb.dimensions();

    thumb
        .save_with_format(cache_path, ImageFormat::Png)
        .map_err(|e| format!("Save thumbnail failed: {e}"))?;

    debug_log(&format!(
        "thumbnail generated: source={:?} cache={:?} size={}x{}",
        path, cache_path, w, h
    ));

    Ok(ThumbnailResponse {
        path: cache_path.to_string_lossy().into_owned(),
        width: w,
        height: h,
        cached: false,
    })
}

fn register_or_wait(
    key: &str,
) -> Option<oneshot::Receiver<Result<ThumbnailResponse, String>>> {
    let mut map = INFLIGHT.lock().expect("inflight poisoned");
    if let Some(waiters) = map.get_mut(key) {
        let (tx, rx) = oneshot::channel::<Result<ThumbnailResponse, String>>();
        waiters.push(tx);
        return Some(rx);
    }
    map.insert(key.to_string(), Vec::new());
    None
}

fn notify_waiters(key: &str, result: Result<ThumbnailResponse, String>) {
    let waiters = {
        let mut map = INFLIGHT.lock().expect("inflight poisoned");
        map.remove(key)
    };
    if let Some(waiters) = waiters {
        for tx in waiters {
            let _ = tx.send(result.clone());
        }
    }
}

fn trim_cache(dir: &Path, max_bytes: u64, max_files: usize) {
    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if let Ok(md) = entry.metadata() {
                let modified = md.modified().unwrap_or(std::time::UNIX_EPOCH);
                entries.push((entry.path(), md.len(), modified));
            }
        }
    }

    let total_bytes: u64 = entries.iter().map(|e| e.1).sum();
    let total_files = entries.len();
    if total_bytes <= max_bytes && total_files <= max_files {
        return;
    }

    // sort by oldest first
    entries.sort_by_key(|e| e.2);
    let mut bytes = total_bytes;
    let mut files = total_files;
    for (path, size, _) in entries {
        if bytes <= max_bytes && files <= max_files {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            bytes = bytes.saturating_sub(size);
            files -= 1;
        }
    }
}

fn decode_with_timeout<R: std::io::BufRead + std::io::Seek + Send + 'static>(
    reader: ImageReader<R>,
    timeout: Duration,
) -> Result<image::DynamicImage, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let res = reader.decode();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(img)) => Ok(img),
        Ok(Err(e)) => Err(format!("Decode failed: {e}")),
        Err(mpsc::RecvTimeoutError::Timeout) => Err("Decode timed out".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("Decode worker crashed".into()),
    }
}

fn sanitize_input_path(raw: &str) -> Result<PathBuf, String> {
    let pb = PathBuf::from(raw);

    // basic poison checks
    let raw_lc = raw.to_lowercase();
    if raw_lc.contains('\0') {
        return Err("Invalid path".into());
    }

    // deny obvious special trees on unix
    #[cfg(not(target_os = "windows"))]
    {
        if raw.starts_with("/proc/") || raw == "/proc" || raw.starts_with("/dev/") || raw == "/dev"
        {
            return Err("Refusing to thumbnail special device/proc files".into());
        }
    }

    // canonicalize to resolve traversal and detect symlinks
    let meta = fs::symlink_metadata(&pb)
        .map_err(|e| format!("Path does not exist or unreadable: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Refusing to thumbnail symlinked files".into());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        // block device/pipe types if needed
        const FILE_TYPE_PIPE: u32 = 0x0000_1000;
        const FILE_TYPE_CHAR: u32 = 0x0000_2000;
        let attrs = meta.file_attributes();
        if attrs & (FILE_TYPE_PIPE | FILE_TYPE_CHAR) != 0 {
            return Err("Refusing to thumbnail special device files".into());
        }
    }

    let canon = pb
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize path: {e}"))?;

    Ok(canon)
}

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
use tauri::AppHandle;
use tauri::Manager;
use tokio::sync::oneshot;

mod thumbnails_svg;
use thumbnails_svg::render_svg_thumbnail;
mod thumbnails_pdf;
use thumbnails_pdf::render_pdf_thumbnail;
mod thumbnails_video;
use thumbnails_video::render_video_thumbnail;

use crate::db;
use crate::fs_utils::debug_log;

#[derive(Debug)]
struct CacheState {
    hot_bytes: u64,
    hot_files: usize,
    cold_bytes: u64,
    cold_files: usize,
    initialized: bool,
    rotating: bool,
}

impl CacheState {
    fn new() -> Self {
        CacheState {
            hot_bytes: 0,
            hot_files: 0,
            cold_bytes: 0,
            cold_files: 0,
            initialized: false,
            rotating: false,
        }
    }
}

#[derive(Clone, Debug)]
struct CachePaths {
    base: PathBuf,
    hot: PathBuf,
    cold: PathBuf,
}

const MAX_DIM_DEFAULT: u32 = 96;
const MAX_DIM_HARD_LIMIT: u32 = 512;
const MIN_DIM_HARD_LIMIT: u32 = 32;
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_FILE_BYTES_VIDEO: u64 = 1_000 * 1024 * 1024; // 1 GB
const POOL_MIN_THREADS: usize = 2;
const POOL_MAX_THREADS: usize = 8;
const CACHE_MAX_FILES: usize = 2000;
const MAX_SOURCE_DIM: u32 = 20000;
const DECODE_TIMEOUT_MS: u64 = 750;
const GLOBAL_HARD_MAX_INFLIGHT: usize = 32;
const CACHE_DEFAULT_MB: u64 = 300;
const CACHE_MIN_MB: u64 = 50;
const CACHE_MAX_MB: u64 = 1000;

fn cache_max_bytes() -> u64 {
    if let Ok(conn) = db::open() {
        if let Ok(Some(s)) = db::get_setting_string(&conn, "thumbCacheMb") {
            if let Ok(n) = s.parse::<u64>() {
                if (CACHE_MIN_MB..=CACHE_MAX_MB).contains(&n) {
                    return n * 1024 * 1024;
                }
            }
        }
    }
    CACHE_DEFAULT_MB * 1024 * 1024
}

fn ensure_cache_dirs() -> Result<CachePaths, String> {
    let base = cache_dir()?;
    let hot = base.join("hot");
    let cold = base.join("cold");
    fs::create_dir_all(&hot).map_err(|e| format!("Failed to create thumbnail cache dir: {e}"))?;
    fs::create_dir_all(&cold).map_err(|e| format!("Failed to create thumbnail cache dir: {e}"))?;
    Ok(CachePaths { base, hot, cold })
}

fn scan_dir_counts(dir: &Path) -> (u64, usize) {
    let mut bytes: u64 = 0;
    let mut files = 0;
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if let Ok(md) = entry.metadata() {
                if md.is_file() {
                    bytes = bytes.saturating_add(md.len());
                    files += 1;
                }
            }
        }
    }
    (bytes, files)
}

fn init_cache_state(paths: &CachePaths, state: &mut CacheState) {
    if state.initialized {
        return;
    }
    let (hot_bytes, hot_files) = scan_dir_counts(&paths.hot);
    let (cold_bytes, cold_files) = scan_dir_counts(&paths.cold);
    state.hot_bytes = hot_bytes;
    state.hot_files = hot_files;
    state.cold_bytes = cold_bytes;
    state.cold_files = cold_files;
    state.initialized = true;
}

fn cache_paths_for_key(paths: &CachePaths, key: &str) -> (PathBuf, PathBuf) {
    let file = format!("{key}.png");
    (paths.hot.join(&file), paths.cold.join(&file))
}

fn remove_existing_for_key(paths: &CachePaths, state: &mut CacheState, key: &str) {
    let (hot_path, cold_path) = cache_paths_for_key(paths, key);
    if let Ok(md) = fs::metadata(&hot_path) {
        let _ = fs::remove_file(&hot_path);
        state.hot_bytes = state.hot_bytes.saturating_sub(md.len());
        state.hot_files = state.hot_files.saturating_sub(1);
    }
    if let Ok(md) = fs::metadata(&cold_path) {
        let _ = fs::remove_file(&cold_path);
        state.cold_bytes = state.cold_bytes.saturating_sub(md.len());
        state.cold_files = state.cold_files.saturating_sub(1);
    }
}

fn cached_thumbnail(paths: &CachePaths, key: &str) -> Option<ThumbnailResponse> {
    let (hot_path, cold_path) = cache_paths_for_key(paths, key);
    if let Some((w, h)) = cached_dims(&hot_path) {
        return Some(ThumbnailResponse {
            path: hot_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
            cached: true,
        });
    }
    if let Some((w, h)) = cached_dims(&cold_path) {
        return Some(ThumbnailResponse {
            path: cold_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
            cached: true,
        });
    }
    None
}

fn maybe_rotate(paths: &CachePaths, state: &mut CacheState, limit_bytes: u64, limit_files: usize) {
    if state.rotating {
        return;
    }
    if state.hot_bytes <= limit_bytes && state.hot_files <= limit_files {
        return;
    }

    state.rotating = true;

    let cold_old = paths.base.join("cold.old");
    let _ = fs::remove_dir_all(&cold_old);
    let _ = fs::rename(&paths.cold, &cold_old);

    if let Err(e) = fs::rename(&paths.hot, &paths.cold) {
        thumb_log(&format!("thumb cache rotate failed (hot->cold): {e}"));
        let _ = fs::create_dir_all(&paths.hot);
        state.rotating = false;
        state.initialized = false; // force rescan next time
        return;
    }

    if let Err(e) = fs::create_dir_all(&paths.hot) {
        thumb_log(&format!("thumb cache recreate hot failed: {e}"));
        state.rotating = false;
        state.initialized = false;
        return;
    }

    // Move counts: previous hot becomes cold; previous cold discarded.
    state.cold_bytes = state.hot_bytes;
    state.cold_files = state.hot_files;
    state.hot_bytes = 0;
    state.hot_files = 0;
    state.rotating = false;

    std::thread::spawn(move || {
        let _ = fs::remove_dir_all(cold_old);
    });
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ThumbKind {
    Image,
    Svg,
    Pdf,
    Video,
}

static POOL_THREADS: Lazy<usize> =
    Lazy::new(|| num_cpus::get().clamp(POOL_MIN_THREADS, POOL_MAX_THREADS));
static THUMB_POOL: Lazy<ThreadPool> = Lazy::new(|| {
    let threads = *POOL_THREADS;
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("thumb-{}", i))
        .build()
        .expect("failed to build thumbnail pool")
});

static INFLIGHT: Lazy<
    std::sync::Mutex<HashMap<String, Vec<oneshot::Sender<Result<ThumbnailResponse, String>>>>>,
> = Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
static LIMITER: Lazy<std::sync::Mutex<ConcurrencyLimiter>> = Lazy::new(|| {
    let threads = *POOL_THREADS;
    std::sync::Mutex::new(ConcurrencyLimiter::new(threads))
});
static LOG_THUMBS: Lazy<bool> =
    Lazy::new(|| std::env::var("BROWSEY_DEBUG_THUMBS").is_ok() || cfg!(debug_assertions));
static CACHE_STATE: Lazy<std::sync::Mutex<CacheState>> =
    Lazy::new(|| std::sync::Mutex::new(CacheState::new()));

#[derive(Serialize, Clone)]
pub struct ThumbnailResponse {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub cached: bool,
}

#[tauri::command]
pub async fn get_thumbnail(
    app_handle: AppHandle,
    path: String,
    max_dim: Option<u32>,
    generation: Option<String>,
) -> Result<ThumbnailResponse, String> {
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
    let kind = thumb_kind(&target);
    let mut ffmpeg_override: Option<PathBuf> = None;
    if matches!(kind, ThumbKind::Video) {
        if let Ok(conn) = db::open() {
            if let Ok(Some(false)) = db::get_setting_bool(&conn, "videoThumbs") {
                return Err("Video thumbnails disabled".to_string());
            }
            if let Ok(Some(path)) = db::get_setting_string(&conn, "ffmpegPath") {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    ffmpeg_override = Some(PathBuf::from(trimmed));
                }
            }
        }
    }
    let size_limit = match kind {
        ThumbKind::Video => MAX_FILE_BYTES_VIDEO,
        _ => MAX_FILE_BYTES,
    };
    if meta.len() > size_limit {
        return Err(format!(
            "File too large for thumbnail (>{} MB)",
            size_limit / 1024 / 1024
        ));
    }
    let mtime = meta.modified().ok();

    let paths = ensure_cache_dirs()?;

    {
        let mut state = CACHE_STATE.lock().expect("cache state poisoned");
        init_cache_state(&paths, &mut state);
    }

    let inflight_limit = limiter_limit_for_kind(kind);

    let key = cache_key(&target, mtime, max_dim);
    if let Some(hit) = cached_thumbnail(&paths, &key) {
        return Ok(hit);
    }

    let (cache_path, _) = cache_paths_for_key(&paths, &key);

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

    {
        let mut state = CACHE_STATE.lock().expect("cache state poisoned");
        init_cache_state(&paths, &mut state);
        remove_existing_for_key(&paths, &mut state, &key);
    }

    let task_path = target.clone();
    let task_cache = cache_path.clone();

    if !try_acquire(kind, inflight_limit) {
        let msg = format!("Too many concurrent thumbnails (limit {inflight_limit})");
        notify_waiters(&key, Err(msg.clone()));
        return Err(msg);
    }

    let res = tauri::async_runtime::spawn_blocking(move || {
        let res_dir_opt = app_handle.path().resource_dir().ok();
        THUMB_POOL.install(|| {
            generate_thumbnail(
                &task_path,
                &task_cache,
                max_dim,
                res_dir_opt.as_deref(),
                generation.as_deref(),
                ffmpeg_override.clone(),
            )
        })
    })
    .await
    .map_err(|e| format!("Thumbnail task cancelled: {e}"));

    release(kind);

    let res = res?;

    let res = match res {
        Ok(r) => {
            if let Ok(md) = fs::metadata(&cache_path) {
                let limit_bytes = cache_max_bytes();
                let mut state = CACHE_STATE.lock().expect("cache state poisoned");
                init_cache_state(&paths, &mut state);
                state.hot_bytes = state.hot_bytes.saturating_add(md.len());
                state.hot_files = state.hot_files.saturating_add(1);
                maybe_rotate(&paths, &mut state, limit_bytes, CACHE_MAX_FILES);
            }
            notify_waiters(&key, Ok(r.clone()));
            Ok(r)
        }
        Err(err) => {
            notify_waiters(&key, Err(err.clone()));
            Err(err)
        }
    };

    res
}

fn try_acquire(kind: ThumbKind, kind_limit: usize) -> bool {
    let mut limiter = LIMITER.lock().expect("limiter poisoned");
    limiter.try_acquire(kind, kind_limit)
}

fn release(kind: ThumbKind) {
    let mut limiter = LIMITER.lock().expect("limiter poisoned");
    limiter.release(kind);
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
    resource_dir: Option<&Path>,
    generation: Option<&str>,
    ffmpeg_override: Option<PathBuf>,
) -> Result<ThumbnailResponse, String> {
    if is_video(path) {
        let (w, h) = render_video_thumbnail(
            path,
            cache_path,
            max_dim,
            generation,
            ffmpeg_override.as_deref(),
        )?;
        return Ok(ThumbnailResponse {
            path: cache_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
            cached: false,
        });
    }

    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
    {
        let (w, h) = render_pdf_thumbnail(path, cache_path, max_dim, resource_dir)?;
        return Ok(ThumbnailResponse {
            path: cache_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
            cached: false,
        });
    }

    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
    {
        let (w, h) = render_svg_thumbnail(path, cache_path, max_dim)?;
        return Ok(ThumbnailResponse {
            path: cache_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
            cached: false,
        });
    }

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

    thumb_log(&format!(
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

pub(super) fn thumb_log(msg: &str) {
    if *LOG_THUMBS {
        debug_log(msg);
    }
}

fn register_or_wait(key: &str) -> Option<oneshot::Receiver<Result<ThumbnailResponse, String>>> {
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
    let meta =
        fs::symlink_metadata(&pb).map_err(|e| format!("Path does not exist or unreadable: {e}"))?;
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

fn thumb_kind(path: &Path) -> ThumbKind {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => ThumbKind::Pdf,
        Some("svg") => ThumbKind::Svg,
        Some("mp4") | Some("mov") | Some("m4v") | Some("webm") | Some("mkv") | Some("avi") => {
            ThumbKind::Video
        }
        _ => ThumbKind::Image,
    }
}

fn is_video(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4") | Some("mov") | Some("m4v") | Some("webm") | Some("mkv") | Some("avi") => true,
        _ => false,
    }
}

fn limiter_limit_for_kind(kind: ThumbKind) -> usize {
    let threads = *POOL_THREADS;
    let base = match kind {
        ThumbKind::Image => threads.saturating_mul(4),
        ThumbKind::Svg | ThumbKind::Pdf => threads.saturating_mul(2),
        ThumbKind::Video => threads.saturating_mul(2).min(4), // hard cap video parallelisme
    };
    base.clamp(POOL_MIN_THREADS, GLOBAL_HARD_MAX_INFLIGHT)
}

#[derive(Debug)]
struct ConcurrencyLimiter {
    global_active: usize,
    image_active: usize,
    doc_active: usize,
    threads: usize,
}

impl ConcurrencyLimiter {
    fn new(threads: usize) -> Self {
        Self {
            global_active: 0,
            image_active: 0,
            doc_active: 0,
            threads,
        }
    }

    fn try_acquire(&mut self, kind: ThumbKind, kind_limit: usize) -> bool {
        let global_limit = GLOBAL_HARD_MAX_INFLIGHT.min(self.threads.saturating_mul(4));
        if self.global_active >= global_limit {
            return false;
        }
        let kind_ok = match kind {
            ThumbKind::Image => self.image_active < kind_limit,
            ThumbKind::Svg | ThumbKind::Pdf | ThumbKind::Video => self.doc_active < kind_limit,
        };
        if !kind_ok {
            return false;
        }
        self.global_active += 1;
        match kind {
            ThumbKind::Image => self.image_active += 1,
            ThumbKind::Svg | ThumbKind::Pdf | ThumbKind::Video => self.doc_active += 1,
        }
        true
    }

    fn release(&mut self, kind: ThumbKind) {
        if self.global_active > 0 {
            self.global_active -= 1;
        }
        match kind {
            ThumbKind::Image => {
                if self.image_active > 0 {
                    self.image_active -= 1;
                }
            }
            ThumbKind::Svg | ThumbKind::Pdf | ThumbKind::Video => {
                if self.doc_active > 0 {
                    self.doc_active -= 1;
                }
            }
        }
    }
}

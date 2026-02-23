use std::fs;
use std::path::{Path, PathBuf};

use blake3::Hasher;
use image::codecs::png::{CompressionType as PngCompression, FilterType as PngFilter, PngEncoder};
use image::{DynamicImage, GrayImage, ImageEncoder, RgbImage};
use image::metadata::Orientation;
use image::ImageDecoder;
use image::ImageReader;
use image::Limits;
use image::{imageops::FilterType, GenericImageView, ImageFormat};
use jpeg_decoder::{Decoder as JpegScaleDecoder, ImageInfo as JpegImageInfo, PixelFormat as JpegPixelFormat};
use once_cell::sync::Lazy;
use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Seek};
use std::sync::mpsc;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Manager;
use tokio::sync::{oneshot, Semaphore};

mod thumbnails_svg;
use thumbnails_svg::render_svg_thumbnail;
mod thumbnails_pdf;
use thumbnails_pdf::render_pdf_thumbnail;
mod thumbnails_video;
use thumbnails_video::render_video_thumbnail;
mod error;

use crate::db;
use crate::errors::api_error::ApiResult;
use crate::fs_utils::debug_log;
use error::{map_api_result, ThumbnailError};

const MAX_DIM_DEFAULT: u32 = 96;
const MAX_DIM_HARD_LIMIT: u32 = 512;
const MIN_DIM_HARD_LIMIT: u32 = 32;
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_FILE_BYTES_VIDEO: u64 = 1_000 * 1024 * 1024; // 1 GB
const POOL_MIN_THREADS: usize = 2;
const POOL_MAX_THREADS: usize = 8;
const CACHE_MAX_FILES: usize = 2000;
const MAX_SOURCE_DIM: u32 = 20000;
const DECODE_TIMEOUT_MS: u64 = 2000;
const DECODE_TIMEOUT_MS_GVFS: u64 = 8000;
const DECODE_TIMEOUT_MS_HDR_EXR: u64 = 6000;
const DECODE_TIMEOUT_MS_GVFS_HDR_EXR: u64 = 12000;
const GLOBAL_HARD_MAX_INFLIGHT: usize = 32;
const CACHE_DEFAULT_MB: u64 = 300;
const CACHE_MIN_MB: u64 = 50;
const CACHE_MAX_MB: u64 = 1000;
const MAX_DECODE_BYTES: u64 = (MAX_SOURCE_DIM as u64) * (MAX_SOURCE_DIM as u64) * 4;
const JPEG_SCALED_DECODE_TARGET_MULTIPLIER: u32 = 4;

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ThumbKind {
    Image,
    Svg,
    Pdf,
    Video,
}

static POOL_THREADS: Lazy<usize> =
    Lazy::new(|| num_cpus::get().clamp(POOL_MIN_THREADS, POOL_MAX_THREADS));
static DECODE_POOL: Lazy<ThreadPool> = Lazy::new(|| {
    let threads = (*POOL_THREADS)
        .saturating_mul(2)
        .clamp(POOL_MIN_THREADS, POOL_MAX_THREADS * 2);
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("thumb-decode-{i}"))
        .build()
        .expect("failed to build decode pool")
});

type InflightWaiters = Vec<oneshot::Sender<Result<ThumbnailResponse, String>>>;
type InflightMap = HashMap<String, InflightWaiters>;
static INFLIGHT: Lazy<std::sync::Mutex<InflightMap>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
static LOG_THUMBS: Lazy<bool> =
    Lazy::new(|| std::env::var("BROWSEY_DEBUG_THUMBS").is_ok() || cfg!(debug_assertions));
static BLOCKING_SEM: Lazy<Semaphore> = Lazy::new(|| {
    let permits = (*POOL_THREADS)
        .saturating_mul(4)
        .clamp(POOL_MIN_THREADS, GLOBAL_HARD_MAX_INFLIGHT);
    Semaphore::new(permits)
});

#[derive(Serialize, Clone)]
pub struct ThumbnailResponse {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub cached: bool,
}

#[derive(Serialize, Clone)]
pub struct ThumbnailCacheClearResult {
    pub removed_files: u64,
    pub removed_bytes: u64,
}

#[tauri::command]
pub fn clear_thumbnail_cache() -> ApiResult<ThumbnailCacheClearResult> {
    map_api_result(clear_thumbnail_cache_impl().map_err(ThumbnailError::from_external_message))
}

fn clear_thumbnail_cache_impl() -> Result<ThumbnailCacheClearResult, String> {
    let dir = cache_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create thumbnail cache dir: {e}"))?;
        return Ok(ThumbnailCacheClearResult {
            removed_files: 0,
            removed_bytes: 0,
        });
    }

    let (removed_files, removed_bytes) = thumbnail_cache_stats(&dir)?;

    fs::remove_dir_all(&dir).map_err(|e| format!("Failed to clear thumbnail cache: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to recreate thumbnail cache dir: {e}"))?;

    Ok(ThumbnailCacheClearResult {
        removed_files,
        removed_bytes,
    })
}

#[tauri::command]
pub async fn get_thumbnail(
    app_handle: AppHandle,
    path: String,
    max_dim: Option<u32>,
    generation: Option<String>,
) -> ApiResult<ThumbnailResponse> {
    map_api_result(
        get_thumbnail_impl(app_handle, path, max_dim, generation)
            .await
            .map_err(ThumbnailError::from_external_message),
    )
}

async fn get_thumbnail_impl(
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

    let permits = if matches!(kind, ThumbKind::Svg | ThumbKind::Pdf | ThumbKind::Video) {
        2
    } else {
        1
    };
    let permit_global = BLOCKING_SEM
        .acquire_many(permits)
        .await
        .map_err(|_| "Semaphore closed".to_string())?;

    let res = tauri::async_runtime::spawn_blocking(move || {
        let res_dir_opt = app_handle.path().resource_dir().ok();
        generate_thumbnail(
            &task_path,
            &task_cache,
            max_dim,
            res_dir_opt.as_deref(),
            generation.as_deref(),
            ffmpeg_override.clone(),
        )
    })
    .await
    .map_err(|e| format!("Thumbnail task cancelled: {e}"));

    if let Err(err) = res.as_ref() {
        // Make sure callers waiting on the same key get released even on panics/JoinError.
        notify_waiters(&key, Err(err.clone()));
    }

    drop(permit_global);

    let res = res?;

    static TRIM_COUNTER: Lazy<std::sync::Mutex<u32>> = Lazy::new(|| std::sync::Mutex::new(0));

    let res = match res {
        Ok(r) => {
            notify_waiters(&key, Ok(r.clone()));
            let mut counter = TRIM_COUNTER.lock().expect("trim counter poisoned");
            *counter = counter.wrapping_add(1);
            if (*counter).is_multiple_of(100) {
                let max_bytes = cache_max_bytes();
                trim_cache(&cache_dir, max_bytes, CACHE_MAX_FILES);
            }
            Ok(r)
        }
        Err(err) => {
            notify_waiters(&key, Err(err.clone()));
            Err(err)
        }
    };

    res
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = dirs_next::cache_dir()
        .or_else(dirs_next::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    Ok(base.join("browsey").join("thumbs"))
}

fn thumbnail_cache_stats(root: &Path) -> Result<(u64, u64), String> {
    let mut dirs = vec![root.to_path_buf()];
    let mut files = 0_u64;
    let mut bytes = 0_u64;

    while let Some(dir) = dirs.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read thumbnail cache dir {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read thumbnail cache entry: {e}"))?;
            let path = entry.path();
            let ty = entry
                .file_type()
                .map_err(|e| format!("Failed to read file type {}: {e}", path.display()))?;

            if ty.is_dir() {
                dirs.push(path);
                continue;
            }
            if ty.is_file() {
                files += 1;
                let len = entry
                    .metadata()
                    .map_err(|e| format!("Failed to read metadata {}: {e}", path.display()))?
                    .len();
                bytes = bytes.saturating_add(len);
            }
        }
    }

    Ok((files, bytes))
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
    if matches!(thumb_kind(path), ThumbKind::Video) {
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
        | ImageFormat::OpenExr
        | ImageFormat::Dds => {}
        _ => return Err("Unsupported image format".into()),
    }

    let timeout = decode_timeout_for_path(path);
    let (img, orientation) = if fmt == ImageFormat::Jpeg {
        match decode_jpeg_scaled_with_timeout(path, max_dim, timeout) {
            Ok(img) => {
                thumb_log(&format!("jpeg scaled decode used: {}", path.display()));
                img
            }
            Err(err) => {
                thumb_log(&format!(
                    "jpeg scaled decode fallback: source={} reason={}",
                    path.display(),
                    err
                ));
                decode_with_timeout(reader, fmt, timeout)?
            }
        }
    } else {
        decode_with_timeout(reader, fmt, timeout)?
    };

    let (src_w, src_h) = img.dimensions();
    if src_w > MAX_SOURCE_DIM || src_h > MAX_SOURCE_DIM {
        return Err("Image dimensions too large for thumbnail".into());
    }
    let mut thumb = img.resize(max_dim, max_dim, FilterType::Nearest);
    if let Some(orientation) = orientation {
        thumb.apply_orientation(orientation);
    }
    let (w, h) = thumb.dimensions();

    // Save quickly: fast compression and no PNG filters to cut CPU time.
    {
        let file =
            fs::File::create(cache_path).map_err(|e| format!("Save thumbnail failed: {e}"))?;
        let writer = std::io::BufWriter::new(file);
        let encoder =
            PngEncoder::new_with_quality(writer, PngCompression::Fast, PngFilter::NoFilter);
        let rgba = thumb.to_rgba8();
        let (w, h) = rgba.dimensions();
        encoder
            .write_image(&rgba, w, h, image::ColorType::Rgba8.into())
            .map_err(|e| format!("Save thumbnail failed: {e}"))?;
    }

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

fn decode_with_timeout<R: BufRead + Seek + Send + 'static>(
    reader: ImageReader<R>,
    format: ImageFormat,
    timeout: Duration,
) -> Result<(image::DynamicImage, Option<Orientation>), String> {
    // Apply codec limits to guard against pathological inputs.
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIM);
    limits.max_image_height = Some(MAX_SOURCE_DIM);
    limits.max_alloc = Some(MAX_DECODE_BYTES);

    // Wrap reader so we can cooperatively abort on timeout.
    let inner = reader.into_inner();
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wrapped = CancelableReader {
        inner,
        cancelled: cancel_flag.clone(),
    };

    let mut reader = ImageReader::with_format(wrapped, format);
    reader.limits(limits);

    let (tx, rx) = mpsc::channel();

    DECODE_POOL.spawn_fifo(move || {
        let res = (|| {
            let mut decoder = reader.into_decoder()?;
            let orientation = decoder.orientation().ok();
            let img = image::DynamicImage::from_decoder(decoder)?;
            Ok::<_, image::ImageError>((img, orientation))
        })();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(img)) => Ok(img),
        Ok(Err(e)) => Err(format!("Decode failed: {e}")),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Err("Decode timed out".into())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("Decode worker crashed".into()),
    }
}

fn decode_jpeg_scaled_with_timeout(
    path: &Path,
    max_dim: u32,
    timeout: Duration,
) -> Result<(DynamicImage, Option<Orientation>), String> {
    let file = fs::File::open(path).map_err(|e| format!("Open failed: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wrapped = CancelableReader {
        inner: reader,
        cancelled: cancel_flag.clone(),
    };

    let (tx, rx) = mpsc::channel();

    DECODE_POOL.spawn_fifo(move || {
        let res = (|| {
            let mut decoder = JpegScaleDecoder::new(wrapped);
            decoder.set_max_decoding_buffer_size(MAX_DECODE_BYTES.min(usize::MAX as u64) as usize);

            decoder
                .read_info()
                .map_err(|e| format!("JPEG scaled decode failed: {e}"))?;

            let src_info = decoder
                .info()
                .ok_or_else(|| "JPEG scaled decode missing metadata".to_string())?;
            if u32::from(src_info.width) > MAX_SOURCE_DIM || u32::from(src_info.height) > MAX_SOURCE_DIM
            {
                return Err("Image dimensions too large for thumbnail".into());
            }

            let req_dim = max_dim
                .saturating_mul(JPEG_SCALED_DECODE_TARGET_MULTIPLIER)
                .clamp(1, u16::MAX as u32) as u16;
            let _ = decoder
                .scale(req_dim, req_dim)
                .map_err(|e| format!("JPEG scaled decode setup failed: {e}"))?;

            let pixels = decoder
                .decode()
                .map_err(|e| format!("JPEG scaled decode failed: {e}"))?;
            let orientation = decoder
                .exif_data()
                .and_then(Orientation::from_exif_chunk);
            let info = decoder
                .info()
                .ok_or_else(|| "JPEG scaled decode missing output metadata".to_string())?;
            let img = jpeg_pixels_to_dynamic_image(pixels, info)?;
            Ok((img, orientation))
        })();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(img)) => Ok(img),
        Ok(Err(e)) => Err(e),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Err("Decode timed out".into())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("Decode worker crashed".into()),
    }
}

fn jpeg_pixels_to_dynamic_image(pixels: Vec<u8>, info: JpegImageInfo) -> Result<DynamicImage, String> {
    let w = u32::from(info.width);
    let h = u32::from(info.height);
    match info.pixel_format {
        JpegPixelFormat::RGB24 => {
            let img =
                RgbImage::from_raw(w, h, pixels).ok_or_else(|| "JPEG RGB buffer size mismatch".to_string())?;
            Ok(DynamicImage::ImageRgb8(img))
        }
        JpegPixelFormat::L8 => {
            let img =
                GrayImage::from_raw(w, h, pixels).ok_or_else(|| "JPEG L8 buffer size mismatch".to_string())?;
            Ok(DynamicImage::ImageLuma8(img))
        }
        // Rare camera/legacy cases; fallback to the existing image crate path for compatibility.
        JpegPixelFormat::L16 | JpegPixelFormat::CMYK32 => {
            Err(format!("JPEG scaled decode unsupported pixel format: {:?}", info.pixel_format))
        }
    }
}

fn decode_timeout_for_path(path: &Path) -> Duration {
    let s = path.to_string_lossy().to_lowercase();
    let is_gvfs = s.contains("/gvfs/mtp:") || s.contains("\\gvfs\\mtp:") || s.contains("/gvfs/");
    let is_hdr_or_exr = matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("hdr") | Some("exr")
    );

    if is_gvfs {
        if is_hdr_or_exr {
            Duration::from_millis(DECODE_TIMEOUT_MS_GVFS_HDR_EXR)
        } else {
            Duration::from_millis(DECODE_TIMEOUT_MS_GVFS)
        }
    } else if is_hdr_or_exr {
        Duration::from_millis(DECODE_TIMEOUT_MS_HDR_EXR)
    } else {
        Duration::from_millis(DECODE_TIMEOUT_MS)
    }
}

/// Reader wrapper that allows cooperative cancellation via an AtomicBool flag.
struct CancelableReader<R> {
    inner: R,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl<R: Read> Read for CancelableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "decode cancelled",
            ));
        }
        self.inner.read(buf)
    }
}

impl<R: BufRead> BufRead for CancelableReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "decode cancelled",
            ));
        }
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt);
    }
}

impl<R: Seek> Seek for CancelableReader<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "decode cancelled",
            ));
        }
        self.inner.seek(pos)
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

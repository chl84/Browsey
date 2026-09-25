use std::fs;
use std::path::{Path, PathBuf};

use blake3::Hasher;
use image::codecs::png::{CompressionType as PngCompression, FilterType as PngFilter, PngEncoder};
use image::metadata::Orientation;
use image::ImageDecoder;
use image::ImageReader;
use image::Limits;
use image::{imageops::FilterType, GenericImageView, ImageFormat};
use image::{DynamicImage, GrayImage, ImageEncoder, RgbImage};
use jpeg_decoder::{
    Decoder as JpegScaleDecoder, ImageInfo as JpegImageInfo, PixelFormat as JpegPixelFormat,
};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::io::{self, BufRead, Read, Seek};
use std::sync::RwLock;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Manager;
use tokio::sync::Semaphore;

mod thumbnails_svg;
use thumbnails_svg::render_svg_thumbnail;
mod thumbnails_pdf;
use thumbnails_pdf::render_pdf_thumbnail;
mod thumbnails_video;
use thumbnails_video::render_video_thumbnail;
mod cache_flow;
mod cloud_source;
mod control;
mod error;

use crate::db;
use crate::errors::api_error::ApiResult;
use crate::errors::domain::ErrorCode;
use crate::fs_utils::debug_log;
use error::{map_api_result, ThumbnailError, ThumbnailErrorCode, ThumbnailResult};

const MAX_DIM_DEFAULT: u32 = 96;
const MAX_DIM_HARD_LIMIT: u32 = 512;
const MIN_DIM_HARD_LIMIT: u32 = 32;
pub(super) const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_FILE_BYTES_VIDEO: u64 = 1_000 * 1024 * 1024; // 1 GB
const POOL_MIN_THREADS: usize = 2;
const POOL_MAX_THREADS: usize = 8;
const MAX_SOURCE_DIM: u32 = 20000;
const DECODE_TIMEOUT_MS: u64 = 2000;
const DECODE_TIMEOUT_MS_GVFS: u64 = 8000;
const DECODE_TIMEOUT_MS_HDR_EXR: u64 = 6000;
const DECODE_TIMEOUT_MS_GVFS_HDR_EXR: u64 = 12000;
const GLOBAL_HARD_MAX_INFLIGHT: usize = 8;
const CACHE_DEFAULT_MB: u64 = 300;
const CACHE_MIN_MB: u64 = 50;
const CACHE_MAX_MB: u64 = 1000;
const MAX_DECODE_BYTES: u64 = (MAX_SOURCE_DIM as u64) * (MAX_SOURCE_DIM as u64) * 4;
const JPEG_SCALED_DECODE_TARGET_MULTIPLIER: u32 = 4;

#[derive(Clone, Debug)]
pub(super) struct ThumbnailRuntimeSettings {
    thumb_cache_mb: u64,
    video_thumbs: bool,
    ffmpeg_path: Option<PathBuf>,
    cloud_thumbs: bool,
}

impl Default for ThumbnailRuntimeSettings {
    fn default() -> Self {
        Self {
            thumb_cache_mb: CACHE_DEFAULT_MB,
            video_thumbs: true,
            ffmpeg_path: None,
            cloud_thumbs: false,
        }
    }
}

impl ThumbnailRuntimeSettings {
    fn cache_max_bytes(&self) -> u64 {
        self.thumb_cache_mb * 1024 * 1024
    }
}

static RUNTIME_SETTINGS: Lazy<RwLock<Option<ThumbnailRuntimeSettings>>> =
    Lazy::new(|| RwLock::new(None));

pub(crate) fn invalidate_runtime_settings_cache() {
    let mut guard = RUNTIME_SETTINGS
        .write()
        .expect("thumbnail runtime settings cache poisoned");
    *guard = None;
}

fn runtime_settings() -> ThumbnailRuntimeSettings {
    {
        let guard = RUNTIME_SETTINGS
            .read()
            .expect("thumbnail runtime settings cache poisoned");
        if let Some(settings) = guard.as_ref() {
            return settings.clone();
        }
    }

    let loaded = load_runtime_settings_from_db();
    let mut guard = RUNTIME_SETTINGS
        .write()
        .expect("thumbnail runtime settings cache poisoned");
    *guard = Some(loaded.clone());
    loaded
}

fn load_runtime_settings_from_db() -> ThumbnailRuntimeSettings {
    let mut settings = ThumbnailRuntimeSettings::default();
    match db::open() {
        Ok(conn) => {
            match db::get_setting_string(&conn, "thumbCacheMb") {
                Ok(Some(value)) => {
                    if let Ok(parsed) = value.parse::<u64>() {
                        if (CACHE_MIN_MB..=CACHE_MAX_MB).contains(&parsed) {
                            settings.thumb_cache_mb = parsed;
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    debug_log(&format!(
                        "thumbnail cache size setting unavailable (code={}): {}",
                        error.code().as_code_str(),
                        error
                    ));
                }
            }
            match db::get_setting_bool(&conn, "videoThumbs") {
                Ok(Some(value)) => settings.video_thumbs = value,
                Ok(None) => {}
                Err(error) => {
                    debug_log(&format!(
                        "videoThumbs setting unavailable (code={}): {}",
                        error.code().as_code_str(),
                        error
                    ));
                }
            }
            match db::get_setting_string(&conn, "ffmpegPath") {
                Ok(Some(path)) => {
                    let trimmed = path.trim();
                    if !trimmed.is_empty() {
                        settings.ffmpeg_path = Some(PathBuf::from(trimmed));
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    debug_log(&format!(
                        "ffmpegPath setting unavailable for thumbnails (code={}): {}",
                        error.code().as_code_str(),
                        error
                    ));
                }
            }
            match db::get_setting_bool(&conn, "cloudThumbs") {
                Ok(Some(value)) => settings.cloud_thumbs = value,
                Ok(None) => {}
                Err(error) => {
                    debug_log(&format!(
                        "cloudThumbs setting unavailable for thumbnails (code={}): {}",
                        error.code().as_code_str(),
                        error
                    ));
                }
            }
        }
        Err(error) => {
            debug_log(&format!(
                "thumbnail runtime settings DB unavailable (code={}): {}",
                error.code().as_code_str(),
                error
            ));
        }
    }
    settings
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum ThumbKind {
    Image,
    Svg,
    Pdf,
    Video,
}

static POOL_THREADS: Lazy<usize> =
    Lazy::new(|| num_cpus::get().clamp(POOL_MIN_THREADS, POOL_MAX_THREADS));

static LOG_THUMBS: Lazy<bool> =
    Lazy::new(|| std::env::var("BROWSEY_DEBUG_THUMBS").is_ok() || cfg!(debug_assertions));
static BLOCKING_SEM: Lazy<Semaphore> = Lazy::new(|| {
    let permits = (*POOL_THREADS)
        .saturating_mul(4)
        .clamp(POOL_MIN_THREADS, GLOBAL_HARD_MAX_INFLIGHT);
    Semaphore::new(permits)
});
// Leave worker capacity for local folders even if a remote filesystem stalls.
static REMOTE_SEM: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(4));

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
    map_api_result(clear_thumbnail_cache_impl())
}

fn clear_thumbnail_cache_impl() -> ThumbnailResult<ThumbnailCacheClearResult> {
    let dir = cache_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| {
            ThumbnailError::from_external_message(format!(
                "Failed to create thumbnail cache dir: {e}"
            ))
        })?;
        return Ok(ThumbnailCacheClearResult {
            removed_files: 0,
            removed_bytes: 0,
        });
    }

    let (removed_files, removed_bytes) = thumbnail_cache_stats(&dir)?;

    fs::remove_dir_all(&dir).map_err(|e| {
        ThumbnailError::from_external_message(format!("Failed to clear thumbnail cache: {e}"))
    })?;
    fs::create_dir_all(&dir).map_err(|e| {
        ThumbnailError::from_external_message(format!(
            "Failed to recreate thumbnail cache dir: {e}"
        ))
    })?;

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
    request_id: Option<String>,
    cancel: tauri::State<'_, crate::tasks::CancelState>,
) -> ApiResult<ThumbnailResponse> {
    let guard = match request_id {
        Some(id) => match cancel.register(id) {
            Ok(guard) => Some(guard),
            Err(error) => {
                return map_api_result(Err(ThumbnailError::from_external_message(
                    error.to_string(),
                )))
            }
        },
        None => None,
    };
    let flag = guard.as_ref().map(|g| g.token()).unwrap_or_default();
    let source_kind = if path.starts_with("rclone://") {
        "cloud"
    } else if path.contains("/gvfs/") {
        "gvfs"
    } else {
        "local"
    };
    let budget = if path.starts_with("rclone://") {
        30
    } else if path.contains("/gvfs/") {
        12
    } else {
        10
    };
    let control = control::Control::new(flag, Duration::from_secs(budget));
    let remote_permit =
        if path.starts_with("rclone://") || path.contains("/gvfs/") || path.starts_with("\\\\") {
            match REMOTE_SEM.try_acquire() {
                Ok(permit) => Some(permit),
                Err(_) => {
                    return map_api_result(Err(ThumbnailError::from_external_message(
                        "Too many concurrent thumbnails",
                    )))
                }
            }
        } else {
            None
        };
    // Keep both admission and I/O off the runtime's async worker threads.
    // Never release a worker permit just because the caller timed out.
    let permit = match BLOCKING_SEM.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            return map_api_result(Err(ThumbnailError::from_external_message(
                "Too many concurrent thumbnails",
            )))
        }
    };
    let worker_control = control.clone();
    let started = std::time::Instant::now();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let _remote_permit = remote_permit;
        let _guard = guard;
        get_thumbnail_sync(app_handle, path, max_dim, generation, &worker_control)
    });
    let result = wait_for_worker(task, &control).await;
    tracing::debug!(
        source_kind,
        elapsed_ms = started.elapsed().as_millis() as u64,
        cached = result.as_ref().is_ok_and(|r| r.cached),
        success = result.is_ok(),
        error_code = result
            .as_ref()
            .err()
            .map(|error| error.code().as_code_str()),
        "thumbnail request completed"
    );
    map_api_result(result)
}

async fn wait_for_worker<T>(
    mut task: tauri::async_runtime::JoinHandle<ThumbnailResult<T>>,
    control: &control::Control,
) -> ThumbnailResult<T> {
    loop {
        match tokio::time::timeout(Duration::from_millis(25), &mut task).await {
            Ok(result) => {
                return result.unwrap_or_else(|error| {
                    Err(ThumbnailError::from_external_message(format!(
                        "Thumbnail task cancelled: {error}"
                    )))
                })
            }
            Err(_) => {
                if let Err(error) = control.check() {
                    control
                        .cancelled
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    return Err(error);
                }
            }
        }
    }
}

fn get_thumbnail_sync(
    app_handle: AppHandle,
    path: String,
    max_dim: Option<u32>,
    _generation: Option<String>,
    control: &control::Control,
) -> ThumbnailResult<ThumbnailResponse> {
    control.check()?;
    let max_dim = max_dim
        .unwrap_or(MAX_DIM_DEFAULT)
        .clamp(MIN_DIM_HARD_LIMIT, MAX_DIM_HARD_LIMIT);
    let settings = runtime_settings();
    let cache_dir = cache_dir()?;
    fs::create_dir_all(&cache_dir).map_err(|e| {
        ThumbnailError::from_external_message(format!("Failed to create thumbnail cache dir: {e}"))
    })?;

    let (target, meta, kind, ffmpeg_override, key) = if path.starts_with("rclone://") {
        let source = cloud_source::precheck_cloud_thumbnail_source(&path, &settings)?;
        control.check()?;
        let key = cloud_source::cache_key_for_cloud_source(&source, max_dim);
        if let Some(response) = cached_response(&cache_dir.join(format!("{key}.png"))) {
            return Ok(response);
        }
        let (target, meta, kind, ffmpeg_override) =
            cloud_source::materialize_cloud_thumbnail_source(
                &app_handle,
                &source,
                &control.cancelled,
            )?;
        (target, meta, kind, ffmpeg_override, key)
    } else {
        let (target, meta, kind, ffmpeg_override) =
            resolve_local_thumbnail_source(&path, &settings)?;
        let key = cache_key(&target, meta.modified().ok(), max_dim);
        (target, meta, kind, ffmpeg_override, key)
    };
    control.check()?;
    let size_limit = if matches!(kind, ThumbKind::Video) {
        MAX_FILE_BYTES_VIDEO
    } else {
        MAX_FILE_BYTES
    };
    if meta.len() > size_limit {
        return Err(ThumbnailError::from_external_message(format!(
            "File too large for thumbnail (>{} MB)",
            size_limit / 1024 / 1024
        )));
    }
    let cache_path = cache_dir.join(format!("{key}.png"));
    if let Some(response) = cached_response(&cache_path) {
        return Ok(response);
    }
    cache_flow::with_key_lock(&key, control, || {
        control.check()?;
        if let Some(response) = cached_response(&cache_path) {
            return Ok(response);
        }
        let pending = cache_flow::pending_path(&cache_path);
        let _cleanup = cache_flow::PendingFile(pending.clone());
        let resource_dir = app_handle.path().resource_dir().ok();
        let mut response = generate_thumbnail(
            &target,
            &pending,
            max_dim,
            resource_dir.as_deref(),
            ffmpeg_override,
            control,
        )?;
        control.check()?;
        fs::rename(&pending, &cache_path).map_err(|e| {
            ThumbnailError::from_external_message(format!("Publish thumbnail failed: {e}"))
        })?;
        response.path = cache_path.to_string_lossy().into_owned();
        cache_flow::schedule_trim(cache_dir.clone(), settings.cache_max_bytes());
        Ok(response)
    })
}

fn cached_response(path: &Path) -> Option<ThumbnailResponse> {
    let (width, height) = cached_dims(path)?;
    cache_flow::touch_cache_entry(path);
    Some(ThumbnailResponse {
        path: path.to_string_lossy().into_owned(),
        width,
        height,
        cached: true,
    })
}

fn resolve_local_thumbnail_source(
    path: &str,
    settings: &ThumbnailRuntimeSettings,
) -> ThumbnailResult<(PathBuf, fs::Metadata, ThumbKind, Option<PathBuf>)> {
    let target = sanitize_input_path(path)?;
    let meta = fs::metadata(&target).map_err(|error| {
        ThumbnailError::from_external_message(format!("Failed to read metadata: {error}"))
    })?;
    if !meta.is_file() {
        return Err(ThumbnailError::from_external_message(
            "Target is not a file",
        ));
    }
    let kind = thumb_kind(&target);
    let ffmpeg_override = if matches!(kind, ThumbKind::Video) {
        if !settings.video_thumbs {
            return Err(ThumbnailError::from_external_message(
                "Video thumbnails disabled",
            ));
        }
        settings.ffmpeg_path.clone()
    } else {
        None
    };
    Ok((target, meta, kind, ffmpeg_override))
}

fn cache_dir() -> ThumbnailResult<PathBuf> {
    let base = dirs_next::cache_dir()
        .or_else(dirs_next::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    Ok(base.join("browsey").join("thumbs"))
}

fn thumbnail_cache_stats(root: &Path) -> ThumbnailResult<(u64, u64)> {
    let mut dirs = vec![root.to_path_buf()];
    let mut files = 0_u64;
    let mut bytes = 0_u64;

    while let Some(dir) = dirs.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| {
            ThumbnailError::from_external_message(format!(
                "Failed to read thumbnail cache dir {}: {e}",
                dir.display()
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                ThumbnailError::from_external_message(format!(
                    "Failed to read thumbnail cache entry: {e}"
                ))
            })?;
            let path = entry.path();
            let ty = entry.file_type().map_err(|e| {
                ThumbnailError::from_external_message(format!(
                    "Failed to read file type {}: {e}",
                    path.display()
                ))
            })?;

            if ty.is_dir() {
                dirs.push(path);
                continue;
            }
            if ty.is_file() {
                files += 1;
                let len = entry
                    .metadata()
                    .map_err(|e| {
                        ThumbnailError::from_external_message(format!(
                            "Failed to read metadata {}: {e}",
                            path.display()
                        ))
                    })?
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
    ffmpeg_override: Option<PathBuf>,
    control: &control::Control,
) -> ThumbnailResult<ThumbnailResponse> {
    control.check()?;
    if matches!(thumb_kind(path), ThumbKind::Video) {
        let (w, h) = render_video_thumbnail(
            path,
            cache_path,
            max_dim,
            ffmpeg_override.as_deref(),
            control,
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
        .map_err(|e| ThumbnailError::from_external_message(format!("Open failed: {e}")))?
        .with_guessed_format()
        .map_err(|e| {
            ThumbnailError::from_external_message(format!("Failed to guess format: {e}"))
        })?;

    // format allowlist (image crate supported set)
    let fmt = reader
        .format()
        .ok_or_else(|| ThumbnailError::from_external_message("Unsupported image format"))?;
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
        _ => {
            return Err(ThumbnailError::from_external_message(
                "Unsupported image format",
            ))
        }
    }

    let timeout = decode_timeout_for_path(path);
    let (img, orientation) = if fmt == ImageFormat::Jpeg {
        match decode_jpeg_scaled_with_timeout(reader.into_inner(), max_dim, timeout, control) {
            Ok(img) => {
                thumb_log(&format!("jpeg scaled decode used: {}", path.display()));
                img
            }
            Err(err) => {
                if err.code() != ThumbnailErrorCode::UnsupportedFormat {
                    return Err(err);
                }
                control.check()?;
                thumb_log(&format!(
                    "jpeg scaled decode fallback: source={} reason={}",
                    path.display(),
                    err
                ));
                let reader = ImageReader::open(path).map_err(|e| {
                    ThumbnailError::from_external_message(format!("Open failed: {e}"))
                })?;
                decode_with_timeout(reader, fmt, timeout, control)?
            }
        }
    } else {
        decode_with_timeout(reader, fmt, timeout, control)?
    };

    let (src_w, src_h) = img.dimensions();
    if src_w > MAX_SOURCE_DIM || src_h > MAX_SOURCE_DIM {
        return Err(ThumbnailError::from_external_message(
            "Image dimensions too large for thumbnail",
        ));
    }
    let mut thumb = img.resize(max_dim, max_dim, FilterType::Nearest);
    if let Some(orientation) = orientation {
        thumb.apply_orientation(orientation);
    }
    let (w, h) = thumb.dimensions();

    // Save quickly: fast compression and no PNG filters to cut CPU time.
    {
        let file = fs::File::create(cache_path).map_err(|e| {
            ThumbnailError::from_external_message(format!("Save thumbnail failed: {e}"))
        })?;
        let writer = std::io::BufWriter::new(file);
        let encoder =
            PngEncoder::new_with_quality(writer, PngCompression::Fast, PngFilter::NoFilter);
        let rgba = thumb.to_rgba8();
        let (w, h) = rgba.dimensions();
        encoder
            .write_image(&rgba, w, h, image::ColorType::Rgba8.into())
            .map_err(|e| {
                ThumbnailError::from_external_message(format!("Save thumbnail failed: {e}"))
            })?;
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

fn decode_with_timeout<R: BufRead + Seek + Send + 'static>(
    reader: ImageReader<R>,
    format: ImageFormat,
    timeout: Duration,
    control: &control::Control,
) -> ThumbnailResult<(DynamicImage, Option<Orientation>)> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIM);
    limits.max_image_height = Some(MAX_SOURCE_DIM);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let decode_control =
        control::Control::new(control.cancelled.clone(), control.remaining(timeout)?);
    let wrapped = CancelableReader {
        inner: reader.into_inner(),
        control: decode_control.clone(),
    };
    let mut reader = ImageReader::with_format(wrapped, format);
    reader.limits(limits);
    let result = (|| {
        let mut decoder = reader.into_decoder()?;
        let orientation = decoder.orientation().ok();
        let image = DynamicImage::from_decoder(decoder)?;
        Ok::<_, image::ImageError>((image, orientation))
    })();
    decode_control.check()?;
    result.map_err(|e| ThumbnailError::from_external_message(format!("Decode failed: {e}")))
}

fn decode_jpeg_scaled_with_timeout<R: BufRead + Seek>(
    reader: R,
    max_dim: u32,
    timeout: Duration,
    control: &control::Control,
) -> ThumbnailResult<(DynamicImage, Option<Orientation>)> {
    let decode_control =
        control::Control::new(control.cancelled.clone(), control.remaining(timeout)?);
    let wrapped = CancelableReader {
        inner: reader,
        control: decode_control.clone(),
    };
    let result = (|| {
        let mut decoder = JpegScaleDecoder::new(wrapped);
        decoder.set_max_decoding_buffer_size(MAX_DECODE_BYTES.min(usize::MAX as u64) as usize);
        decoder.read_info().map_err(|e| {
            ThumbnailError::from_external_message(format!("JPEG scaled decode failed: {e}"))
        })?;
        let src = decoder.info().ok_or_else(|| {
            ThumbnailError::from_external_message("JPEG scaled decode missing metadata")
        })?;
        if u32::from(src.width) > MAX_SOURCE_DIM || u32::from(src.height) > MAX_SOURCE_DIM {
            return Err(ThumbnailError::from_external_message(
                "Image dimensions too large for thumbnail",
            ));
        }
        let requested = max_dim
            .saturating_mul(JPEG_SCALED_DECODE_TARGET_MULTIPLIER)
            .clamp(1, u16::MAX as u32) as u16;
        decoder.scale(requested, requested).map_err(|e| {
            ThumbnailError::from_external_message(format!("JPEG scaled decode setup failed: {e}"))
        })?;
        let pixels = decoder.decode().map_err(|e| {
            ThumbnailError::from_external_message(format!("JPEG scaled decode failed: {e}"))
        })?;
        let orientation = decoder.exif_data().and_then(Orientation::from_exif_chunk);
        let info = decoder.info().ok_or_else(|| {
            ThumbnailError::from_external_message("JPEG scaled decode missing output metadata")
        })?;
        Ok((jpeg_pixels_to_dynamic_image(pixels, info)?, orientation))
    })();
    decode_control.check()?;
    result
}

fn jpeg_pixels_to_dynamic_image(
    pixels: Vec<u8>,
    info: JpegImageInfo,
) -> ThumbnailResult<DynamicImage> {
    let w = u32::from(info.width);
    let h = u32::from(info.height);
    match info.pixel_format {
        JpegPixelFormat::RGB24 => {
            let img = RgbImage::from_raw(w, h, pixels).ok_or_else(|| {
                ThumbnailError::from_external_message("JPEG RGB buffer size mismatch")
            })?;
            Ok(DynamicImage::ImageRgb8(img))
        }
        JpegPixelFormat::L8 => {
            let img = GrayImage::from_raw(w, h, pixels).ok_or_else(|| {
                ThumbnailError::from_external_message("JPEG L8 buffer size mismatch")
            })?;
            Ok(DynamicImage::ImageLuma8(img))
        }
        // Rare camera/legacy cases; fallback to the existing image crate path for compatibility.
        JpegPixelFormat::L16 | JpegPixelFormat::CMYK32 => Err(ThumbnailError::new(
            ThumbnailErrorCode::UnsupportedFormat,
            format!(
                "JPEG scaled decode unsupported pixel format: {:?}",
                info.pixel_format
            ),
        )),
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
    control: control::Control,
}

impl<R: Read> Read for CancelableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.control.check().is_err() {
            return Err(io::Error::other("decode cancelled"));
        }
        self.inner.read(buf)
    }
}

impl<R: BufRead> BufRead for CancelableReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.control.check().is_err() {
            return Err(io::Error::other("decode cancelled"));
        }
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt);
    }
}

impl<R: Seek> Seek for CancelableReader<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        if self.control.check().is_err() {
            return Err(io::Error::other("decode cancelled"));
        }
        self.inner.seek(pos)
    }
}

fn sanitize_input_path(raw: &str) -> ThumbnailResult<PathBuf> {
    let pb = PathBuf::from(raw);

    // basic poison checks
    let raw_lc = raw.to_lowercase();
    if raw_lc.contains('\0') {
        return Err(ThumbnailError::from_external_message("Invalid path"));
    }

    // deny obvious special trees on unix
    #[cfg(not(target_os = "windows"))]
    {
        if raw.starts_with("/proc/") || raw == "/proc" || raw.starts_with("/dev/") || raw == "/dev"
        {
            return Err(ThumbnailError::from_external_message(
                "Refusing to thumbnail special device/proc files",
            ));
        }
    }

    // canonicalize to resolve traversal and detect symlinks
    let meta = fs::symlink_metadata(&pb).map_err(|e| {
        ThumbnailError::from_external_message(format!("Path does not exist or unreadable: {e}"))
    })?;
    if meta.file_type().is_symlink() {
        return Err(ThumbnailError::from_external_message(
            "Refusing to thumbnail symlinked files",
        ));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        // block device/pipe types if needed
        const FILE_TYPE_PIPE: u32 = 0x0000_1000;
        const FILE_TYPE_CHAR: u32 = 0x0000_2000;
        let attrs = meta.file_attributes();
        if attrs & (FILE_TYPE_PIPE | FILE_TYPE_CHAR) != 0 {
            return Err(ThumbnailError::from_external_message(
                "Refusing to thumbnail special device files",
            ));
        }
    }

    let canon = pb.canonicalize().map_err(|e| {
        ThumbnailError::from_external_message(format!("Failed to canonicalize path: {e}"))
    })?;

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

#[cfg(test)]
mod tests {
    use super::{
        invalidate_runtime_settings_cache, runtime_settings, ThumbnailRuntimeSettings,
        RUNTIME_SETTINGS,
    };

    fn fixture_dir() -> std::path::PathBuf {
        static SEQUENCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "browsey-thumb-test-{}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn stalled_io_times_out_without_releasing_its_worker_permit() {
        use std::sync::{mpsc, Arc};
        use std::time::Duration;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore.clone().try_acquire_owned().unwrap();
        let (release, wait) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let task = tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            drop(_permit);
            finished.send(()).unwrap();
            Ok(())
        });
        let control = super::control::Control::new(Default::default(), Duration::from_millis(5));
        let result = tauri::async_runtime::block_on(super::wait_for_worker(task, &control));
        assert_eq!(
            result.unwrap_err().code(),
            super::ThumbnailErrorCode::TimedOut
        );
        assert_eq!(semaphore.available_permits(), 0);
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(semaphore.available_permits(), 1);
    }

    #[test]
    fn cancelled_reader_is_not_retried_as_an_interrupted_read() {
        use std::io::Read;
        let control = super::control::Control::new(
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            std::time::Duration::from_secs(1),
        );
        let mut reader = super::CancelableReader {
            inner: std::io::Cursor::new(vec![1, 2, 3]),
            control,
        };
        assert_eq!(
            reader.read(&mut [0; 2]).unwrap_err().kind(),
            std::io::ErrorKind::Other
        );
    }

    #[test]
    fn jpeg_timeout_is_terminal_not_a_full_decode_fallback() {
        let control = super::control::Control::new(Default::default(), std::time::Duration::ZERO);
        let error = super::decode_jpeg_scaled_with_timeout(
            std::io::Cursor::new(Vec::<u8>::new()),
            96,
            std::time::Duration::from_secs(8),
            &control,
        )
        .unwrap_err();
        assert_eq!(error.code(), super::ThumbnailErrorCode::TimedOut);
        assert_ne!(error.code(), super::ThumbnailErrorCode::UnsupportedFormat);
    }

    #[test]
    fn local_jpeg_decode_and_warm_cache_preserve_dimensions() {
        let dir = fixture_dir();
        let source = dir.join("source.jpg");
        let cache = dir.join("cache.png");
        image::RgbImage::from_pixel(1920, 1080, image::Rgb([24, 128, 200]))
            .save(&source)
            .unwrap();
        let control =
            super::control::Control::new(Default::default(), std::time::Duration::from_secs(10));
        let start = std::time::Instant::now();
        let generated =
            super::generate_thumbnail(&source, &cache, 96, None, None, &control).unwrap();
        let cold = start.elapsed();
        let start = std::time::Instant::now();
        let cached = super::cached_response(&cache).unwrap();
        let warm = start.elapsed();
        assert_eq!((generated.width, generated.height), (96, 54));
        assert!(cached.cached);
        assert_eq!((cached.width, cached.height), (96, 54));
        eprintln!("synthetic local JPEG: cold={cold:?}, warm={warm:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cache_validation_does_not_open_source_and_still_rejects_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = fixture_dir();
        let source = dir.join("source.jpg");
        std::fs::write(&source, b"not read when checking metadata").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert!(super::resolve_local_thumbnail_source(
            source.to_str().unwrap(),
            &ThumbnailRuntimeSettings::default()
        )
        .is_ok());
        let link = dir.join("link.jpg");
        symlink(&source, &link).unwrap();
        assert!(super::resolve_local_thumbnail_source(
            link.to_str().unwrap(),
            &ThumbnailRuntimeSettings::default()
        )
        .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn runtime_settings_cache_invalidation_clears_cached_snapshot() {
        {
            let mut guard = RUNTIME_SETTINGS
                .write()
                .expect("thumbnail runtime settings cache poisoned");
            *guard = Some(ThumbnailRuntimeSettings {
                thumb_cache_mb: 777,
                video_thumbs: false,
                ffmpeg_path: Some("/tmp/ffmpeg-test".into()),
                cloud_thumbs: true,
            });
        }

        let cached = runtime_settings();
        assert_eq!(cached.thumb_cache_mb, 777);
        assert!(!cached.video_thumbs);
        assert!(cached.cloud_thumbs);

        invalidate_runtime_settings_cache();
        let guard = RUNTIME_SETTINGS
            .read()
            .expect("thumbnail runtime settings cache poisoned");
        assert!(
            guard.is_none(),
            "cache should be cleared after invalidation"
        );
    }
}

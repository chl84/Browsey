use super::*;
use crate::commands::thumbnails::{
    cached_response, generate_thumbnail, thumb_kind, validate_source_size, ThumbKind,
    MAX_FILE_BYTES,
};
use image::{codecs::jpeg::JpegEncoder, GenericImageView};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

fn control() -> Control {
    Control::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(5))
}

fn resource(id: u16, name: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = b"8BIM".to_vec();
    out.extend(id.to_be_bytes());
    out.push(name.len() as u8);
    out.extend(name);
    if !(name.len() + 1).is_multiple_of(2) {
        out.push(0);
    }
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(data);
    if !data.len().is_multiple_of(2) {
        out.push(0);
    }
    out
}

fn psd(resources: &[u8]) -> Vec<u8> {
    let mut out = b"8BPS".to_vec();
    out.extend(1u16.to_be_bytes());
    out.extend([0; 6]);
    out.extend(4u16.to_be_bytes());
    out.extend(8000u32.to_be_bytes()); // Large canvas; only the preview is decoded.
    out.extend(12000u32.to_be_bytes());
    out.extend(16u16.to_be_bytes());
    out.extend(4u16.to_be_bytes()); // CMYK canvas doesn't change preview RGB.
    out.extend(3u32.to_be_bytes());
    out.extend(b"xyz");
    out.extend((resources.len() as u32).to_be_bytes());
    out.extend(resources);
    // Deliberately uninterpretable layer/composite data: thumbnails must skip it.
    out.extend(b"this is not raster or layer data");
    out
}

fn preview(format: u32, width: u32, height: u32, bytes: &[u8]) -> Vec<u8> {
    let stride = (width * 24).div_ceil(32) * 4;
    let mut out = Vec::new();
    for value in [
        format,
        width,
        height,
        stride,
        stride * height,
        bytes.len() as u32,
    ] {
        out.extend(value.to_be_bytes());
    }
    out.extend(24u16.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend(bytes);
    out
}

fn jpeg() -> Vec<u8> {
    let image = RgbImage::from_pixel(80, 40, image::Rgb([240, 20, 10]));
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 95)
        .encode_image(&image)
        .unwrap();
    preview(1, 80, 40, &bytes)
}

fn decode(bytes: &[u8]) -> ThumbnailResult<DynamicImage> {
    read_preview(Cursor::new(bytes), bytes.len() as u64, &control())
}

#[test]
fn jpeg_preview_skips_unrelated_padded_resources_and_canvas_layers() {
    let resources = [
        resource(1000, b"ab", b"odd"),
        resource(1036, b"name", &jpeg()),
    ]
    .concat();
    let image = decode(&psd(&resources)).unwrap().to_rgb8();
    assert_eq!(image.dimensions(), (80, 40));
    let pixel = image.get_pixel(20, 10);
    assert!(pixel[0] > 220 && pixel[1] < 40 && pixel[2] < 40);
}

#[test]
fn raw_rgb_and_legacy_bgr_previews_strip_row_padding() {
    for (id, pixel) in [(1036, [240, 20, 10]), (1033, [10, 20, 240])] {
        let bytes = [pixel.as_slice(), &[99], pixel.as_slice(), &[99]].concat();
        let data = preview(0, 1, 2, &bytes);
        let image = decode(&psd(&resource(id, b"", &data))).unwrap().to_rgb8();
        assert_eq!(image.dimensions(), (1, 2));
        assert_eq!(image.as_raw(), &[240, 20, 10, 240, 20, 10]);
    }
}

#[test]
fn legacy_jpeg_swaps_color_channels_and_modern_preview_takes_precedence() {
    let old = resource(1033, b"", &jpeg());
    let old_image = decode(&psd(&old)).unwrap().to_rgb8();
    assert!(old_image.get_pixel(0, 0)[2] > 220);
    let modern = resource(1036, b"", &jpeg());
    let image = decode(&psd(&[old, modern].concat())).unwrap().to_rgb8();
    assert!(image.get_pixel(0, 0)[0] > 220);
}

#[test]
fn missing_preview_returns_the_standard_unsupported_error() {
    assert_eq!(
        decode(&psd(&resource(1000, b"", b"metadata")))
            .unwrap_err()
            .code(),
        ThumbnailErrorCode::UnsupportedFormat
    );
    let unknown = resource(1036, b"", &preview(2, 1, 1, &[0; 4]));
    assert_eq!(
        decode(&psd(&unknown)).unwrap_err().code(),
        ThumbnailErrorCode::UnsupportedFormat
    );
}

#[test]
fn truncated_or_out_of_bounds_resources_fail_before_decoding() {
    let bytes = psd(&resource(1036, b"", &jpeg()));
    let resources_end = bytes.len() - b"this is not raster or layer data".len();
    for end in 0..resources_end {
        assert!(decode(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut bad = bytes.clone();
    bad[33..37].copy_from_slice(&u32::MAX.to_be_bytes()); // Resource section length.
    assert!(decode(&bad).is_err());
    let mut bad = bytes.clone();
    bad[45..49].copy_from_slice(&u32::MAX.to_be_bytes()); // Individual resource size.
    assert!(decode(&bad).is_err());
    let mut bad = bytes;
    bad[..4].copy_from_slice(b"JPEG");
    assert!(decode(&bad).is_err());
}

#[test]
fn preview_limits_and_jpeg_dimensions_are_checked_before_pixel_allocation() {
    let valid = jpeg();
    for (offset, value) in [
        (4, 0),
        (4, MAX_PREVIEW_DIM + 1),
        (12, 1),
        (16, 1),
        (20, u32::MAX),
    ] {
        let mut bad = valid.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(decode(&psd(&resource(1036, b"", &bad))).is_err());
    }
    let mut mismatch = valid.clone();
    mismatch[4..8].copy_from_slice(&1u32.to_be_bytes());
    mismatch[12..16].copy_from_slice(&4u32.to_be_bytes());
    mismatch[16..20].copy_from_slice(&160u32.to_be_bytes());
    assert!(decode(&psd(&resource(1036, b"", &mismatch))).is_err());
    let oversized = preview(0, MAX_PREVIEW_DIM, MAX_PREVIEW_DIM, &[0]);
    assert!(decode(&psd(&resource(1036, b"", &oversized))).is_err());
    let mut bomb = valid;
    let sof = bomb
        .windows(2)
        .position(|bytes| bytes == [0xff, 0xc0])
        .unwrap();
    bomb[sof + 5..sof + 7].copy_from_slice(&20000u16.to_be_bytes());
    bomb[sof + 7..sof + 9].copy_from_slice(&20000u16.to_be_bytes());
    assert!(decode(&psd(&resource(1036, b"", &bomb))).is_err());
}

struct CountedReader {
    inner: Cursor<Vec<u8>>,
    bytes_read: Arc<AtomicU64>,
    cancel_after_read: Option<Arc<AtomicBool>>,
}
impl Read for CountedReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(bytes)?;
        self.bytes_read.fetch_add(count as u64, Ordering::Relaxed);
        if let Some(cancel) = &self.cancel_after_read {
            cancel.store(true, Ordering::Relaxed);
        }
        Ok(count)
    }
}
impl Seek for CountedReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn cancellation_and_deadlines_stop_resource_reads() {
    let bytes = psd(&resource(1036, b"", &jpeg()));
    let len = bytes.len() as u64;
    let cancelled = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicU64::new(0));
    let reader = CountedReader {
        inner: Cursor::new(bytes.clone()),
        bytes_read: reads.clone(),
        cancel_after_read: Some(cancelled.clone()),
    };
    let control = Control::new(cancelled, Duration::from_secs(5));
    assert_eq!(
        read_preview(reader, len, &control).unwrap_err().code(),
        ThumbnailErrorCode::Cancelled
    );
    assert_eq!(reads.load(Ordering::Relaxed), 26);
    let control = Control::new(Arc::new(AtomicBool::new(false)), Duration::ZERO);
    assert_eq!(
        read_preview(Cursor::new(bytes), len, &control)
            .unwrap_err()
            .code(),
        ThumbnailErrorCode::TimedOut
    );
}

#[test]
fn unknown_metadata_is_skipped_without_reading_its_payload() {
    let resources = [
        resource(1000, b"", &vec![0; 2 * 1024 * 1024]),
        resource(1036, b"", &jpeg()),
    ]
    .concat();
    let bytes = psd(&resources);
    let len = bytes.len() as u64;
    let reads = Arc::new(AtomicU64::new(0));
    let reader = CountedReader {
        inner: Cursor::new(bytes),
        bytes_read: reads.clone(),
        cancel_after_read: None,
    };
    read_preview(reader, len, &control()).unwrap();
    assert!(reads.load(Ordering::Relaxed) < 4096);
}

#[test]
fn large_local_psd_uses_shared_png_generation_and_cache_dimensions() {
    let dir = std::env::temp_dir().join(format!(
        "browsey-psd-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let source = dir.join("source.PSD");
    let output = dir.join("thumbnail.png");
    fs::write(&source, psd(&resource(1036, b"", &jpeg()))).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap()
        .set_len(MAX_FILE_BYTES + 1)
        .unwrap();
    assert_eq!(thumb_kind(&source), ThumbKind::Psd);
    validate_source_size(ThumbKind::Psd, fs::metadata(&source).unwrap().len()).unwrap();
    assert_eq!(
        validate_source_size(ThumbKind::Image, MAX_FILE_BYTES + 1)
            .unwrap_err()
            .code(),
        ThumbnailErrorCode::InvalidInput
    );
    let generated = generate_thumbnail(&source, &output, 32, None, None, &control()).unwrap();
    assert_eq!((generated.width, generated.height), (32, 16));
    assert_eq!(image::open(&output).unwrap().dimensions(), (32, 16));
    let cached = cached_response(&output).unwrap();
    assert!(cached.cached);
    assert_eq!((cached.width, cached.height), (32, 16));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires an explicitly supplied PSD file via BROWSEY_PSD_TEST_FILE"]
fn supplied_psd_generates_a_real_thumbnail_without_changing_the_source() {
    let source = std::path::PathBuf::from(
        std::env::var_os("BROWSEY_PSD_TEST_FILE").expect("PSD fixture path"),
    );
    let hash = || {
        let mut hasher = blake3::Hasher::new();
        hasher
            .update_reader(fs::File::open(&source).unwrap())
            .unwrap();
        hasher.finalize()
    };
    let before = hash();
    let dir = std::env::temp_dir().join(format!("browsey-psd-validation-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let output = dir.join("thumbnail.png");
    let result = generate_thumbnail(&source, &output, 96, None, None, &control()).unwrap();
    assert!(result.width > 0 && result.height > 0 && result.width <= 96 && result.height <= 96);
    assert_eq!(
        image::open(&output).unwrap().dimensions(),
        (result.width, result.height)
    );
    assert_eq!(hash(), before);
    println!(
        "PSD thumbnail validation: {}x{} at {}",
        result.width,
        result.height,
        output.display()
    );
}

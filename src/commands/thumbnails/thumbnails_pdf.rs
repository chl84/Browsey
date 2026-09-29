use std::fs::File;
use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::ImageEncoder;
use pdfium_render::prelude::*;

use super::{
    error::{ThumbnailError, ThumbnailResult},
    thumb_log,
};

pub fn render_pdf_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
    resource_dir: Option<&Path>,
) -> ThumbnailResult<(u32, u32)> {
    let pdfium = crate::pdfium_runtime::pdfium(resource_dir)
        .map_err(|e| ThumbnailError::from_external_message(format!("Pdfium load failed: {e}")))?;

    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| ThumbnailError::from_external_message(format!("PDF load failed: {e}")))?;

    let page = doc.pages().get(0).map_err(|e| {
        ThumbnailError::from_external_message(format!("PDF first page failed: {e}"))
    })?;

    // Scale to fit max_dim while keeping aspect
    let dims = (page.width().value, page.height().value);
    let max_side = dims.0.max(dims.1);
    let scale = (max_dim as f32 / max_side).min(1.0);
    let target_w = ((dims.0 * scale).round() as i32).max(1);
    let target_h = ((dims.1 * scale).round() as i32).max(1);

    let render = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(target_w)
                .set_target_height(target_h)
                .rotate_if_landscape(PdfPageRenderRotation::None, false),
        )
        .map_err(|e| ThumbnailError::from_external_message(format!("PDF render failed: {e}")))?;

    let image = render.as_image().map_err(|e| {
        ThumbnailError::from_external_message(format!("PDF image conversion failed: {e}"))
    })?;
    let rgba = image.to_rgba8();
    let file = File::create(cache_path).map_err(|e| {
        ThumbnailError::from_external_message(format!("Save PDF thumbnail failed (open): {e}"))
    })?;
    let encoder = PngEncoder::new_with_quality(file, CompressionType::Fast, FilterType::NoFilter);
    encoder
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ColorType::Rgba8.into(),
        )
        .map_err(|e| {
            ThumbnailError::from_external_message(format!("Save PDF thumbnail failed: {e}"))
        })?;

    thumb_log(&format!(
        "pdf thumbnail generated: source={} cache={} size={}x{}",
        path.display(),
        cache_path.display(),
        image.width(),
        image.height()
    ));

    Ok((image.width(), image.height()))
}

#[cfg(test)]
#[path = "thumbnails_pdf_tests.rs"]
mod tests;

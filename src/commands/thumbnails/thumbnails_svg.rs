use super::{
    error::{ThumbnailError, ThumbnailResult},
    thumb_log,
};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::Tree;
use std::path::Path;

fn contains_ascii_nocase(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

fn has_unsupported_arithmetic_composite_filter(data: &[u8]) -> bool {
    // Guardrail: resvg 0.46 can panic on some malformed/complex feComposite arithmetic filters.
    // If these tokens are present together, skip thumbnail generation for this file.
    contains_ascii_nocase(data, b"fecomposite")
        && contains_ascii_nocase(data, b"operator")
        && contains_ascii_nocase(data, b"arithmetic")
}

pub fn render_svg_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
) -> ThumbnailResult<(u32, u32)> {
    let data = std::fs::read(path)
        .map_err(|e| ThumbnailError::from_external_message(format!("Read SVG failed: {e}")))?;
    if has_unsupported_arithmetic_composite_filter(&data) {
        thumb_log(&format!(
            "svg thumbnail skipped (unsupported feComposite arithmetic filter): {}",
            path.display()
        ));
        return Err(ThumbnailError::from_external_message(
            "SVG uses unsupported arithmetic composite filter",
        ));
    }

    let opt = crate::svg_options::usvg_options_for_path(path);

    let tree = Tree::from_data(&data, &opt)
        .map_err(|e| ThumbnailError::from_external_message(format!("SVG parse failed: {e}")))?;
    let size = tree.size();
    if size.width() == 0.0 || size.height() == 0.0 {
        return Err(ThumbnailError::from_external_message("SVG has zero size"));
    }

    let max_side = size.width().max(size.height()) as f32;
    let scale = (max_dim as f32 / max_side).min(1.0);
    let target_w = (size.width() as f32 * scale).round() as u32;
    let target_h = (size.height() as f32 * scale).round() as u32;
    if target_w == 0 || target_h == 0 {
        return Err(ThumbnailError::from_external_message(
            "SVG scaled size is zero",
        ));
    }

    let mut pixmap = Pixmap::new(target_w, target_h)
        .ok_or_else(|| ThumbnailError::from_external_message("Failed to allocate pixmap"))?;
    let transform = Transform::from_scale(scale, scale);
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, transform, &mut pixmap_mut);

    pixmap.save_png(cache_path).map_err(|e| {
        ThumbnailError::from_external_message(format!("Save SVG thumbnail failed: {e}"))
    })?;

    Ok((target_w, target_h))
}

#[cfg(test)]
mod tests {
    use super::has_unsupported_arithmetic_composite_filter;

    #[test]
    fn detects_fecomposite_arithmetic_filter() {
        let svg = br#"<svg><filter id='f'><feComposite operator="arithmetic"/></filter></svg>"#;
        assert!(has_unsupported_arithmetic_composite_filter(svg));
    }

    #[test]
    fn ignores_regular_svg_without_arithmetic_filter() {
        let svg = br#"<svg><rect width="10" height="10"/></svg>"#;
        assert!(!has_unsupported_arithmetic_composite_filter(svg));
    }
}

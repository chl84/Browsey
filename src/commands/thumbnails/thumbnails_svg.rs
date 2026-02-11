use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::Tree;
use std::path::Path;

pub fn render_svg_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
) -> Result<(u32, u32), String> {
    let data = std::fs::read(path).map_err(|e| format!("Read SVG failed: {e}"))?;

    let opt = crate::svg_options::usvg_options_for_path(path);

    let tree = Tree::from_data(&data, &opt).map_err(|e| format!("SVG parse failed: {e}"))?;
    let size = tree.size();
    if size.width() == 0.0 || size.height() == 0.0 {
        return Err("SVG has zero size".into());
    }

    let max_side = size.width().max(size.height()) as f32;
    let scale = (max_dim as f32 / max_side).min(1.0);
    let target_w = (size.width() as f32 * scale).round() as u32;
    let target_h = (size.height() as f32 * scale).round() as u32;
    if target_w == 0 || target_h == 0 {
        return Err("SVG scaled size is zero".into());
    }

    let mut pixmap = Pixmap::new(target_w, target_h).ok_or("Failed to allocate pixmap")?;
    let transform = Transform::from_scale(scale, scale);
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, transform, &mut pixmap_mut);

    pixmap
        .save_png(cache_path)
        .map_err(|e| format!("Save SVG thumbnail failed: {e}"))?;

    Ok((target_w, target_h))
}

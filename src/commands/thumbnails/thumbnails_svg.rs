use super::error::{ThumbnailError, ThumbnailResult};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::Tree;
use std::path::Path;

pub fn render_svg_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
) -> ThumbnailResult<(u32, u32)> {
    let data = std::fs::read(path)
        .map_err(|e| ThumbnailError::from_external_message(format!("Read SVG failed: {e}")))?;
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
    use super::render_svg_thumbnail;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn render(svg: &str, max_dim: u32) -> image::RgbaImage {
        let dir = std::env::temp_dir().join(format!(
            "browsey-svg-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let input = dir.join("input.svg");
        let output = dir.join("thumbnail.png");
        fs::write(&input, svg).unwrap();
        let size = render_svg_thumbnail(&input, &output, max_dim).unwrap();
        let image = image::open(&output).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), size);
        fs::remove_dir_all(dir).unwrap();
        image
    }

    #[test]
    fn arithmetic_filter_with_oversized_region_renders_without_panicking() {
        // resvg 0.48 fixes the clamped-layer arithmetic filter panic. Exercise
        // the real thumbnail path instead of keeping the old token blacklist.
        let image = render(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
          <defs><filter id="f" x="-100%" y="-100%" width="300%" height="300%">
            <feComposite in="SourceGraphic" in2="SourceGraphic" operator="arithmetic" k2="1"/>
          </filter></defs>
          <rect width="200" height="100" fill="red" filter="url(#f)"/>
        </svg>"#,
            64,
        );
        assert_eq!(image.dimensions(), (64, 32));
        assert!(image.get_pixel(32, 16)[0] > 200);
        assert!(image.get_pixel(32, 16)[3] > 200);
    }

    #[test]
    fn missing_height_uses_viewbox_aspect_ratio_and_nested_transform() {
        let image = render(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" viewBox="0 0 200 100">
          <svg x="0" y="0" width="100" height="100" transform="translate(100 0)">
            <rect width="100" height="100" fill="red"/>
          </svg>
        </svg>"#,
            100,
        );
        assert_eq!(image.dimensions(), (100, 50));
        assert_eq!(image.get_pixel(20, 25)[3], 0);
        assert!(image.get_pixel(75, 25)[0] > 200);
        assert!(image.get_pixel(75, 25)[3] > 200);
    }

    #[test]
    fn system_font_text_renders_with_the_maintained_font_stack() {
        let options = crate::svg_options::usvg_options_for_path(std::path::Path::new("text.svg"));
        let face = options
            .fontdb
            .faces()
            .next()
            .expect("system fonts required for SVG text");
        let family = face.families[0]
            .0
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;");
        let image = render(
            &format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="60">
          <text x="5" y="45" font-size="36" font-family="{family}" fill="black">Browsey 123</text>
        </svg>"#
            ),
            300,
        );
        assert!(image.pixels().filter(|p| p[3] > 128).count() > 100);
    }
}

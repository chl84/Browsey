use std::path::{Path, PathBuf};

use pdfium_render::prelude::*;

use super::thumb_log;

pub fn render_pdf_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
    resource_dir: Option<&Path>,
) -> Result<(u32, u32), String> {
    let bindings = load_pdfium_bindings(resource_dir)?;
    thumb_log(&format!("pdfium: bindings loaded for {}", path.display()));
    let pdfium = Pdfium::new(bindings);

    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF load failed: {e}"))?;

    let page = doc
        .pages()
        .get(0)
        .map_err(|e| format!("PDF first page failed: {e}"))?;

    // Scale to fit max_dim while keeping aspect
    let dims = (page.width().value, page.height().value);
    let max_side = dims.0.max(dims.1);
    let scale = (max_dim as f32 / max_side).min(1.0);
    let target_w = (dims.0 * scale).round() as i32;
    let target_h = (dims.1 * scale).round() as i32;

    let render = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(target_w)
                .set_target_height(target_h)
                .rotate_if_landscape(PdfPageRenderRotation::None, false),
        )
        .map_err(|e| format!("PDF render failed: {e}"))?;

    let image = render.as_image();

    image
        .save_with_format(cache_path, image::ImageFormat::Png)
        .map_err(|e| format!("Save PDF thumbnail failed: {e}"))?;

    thumb_log(&format!(
        "pdf thumbnail generated: source={} cache={} size={}x{}",
        path.display(),
        cache_path.display(),
        image.width(),
        image.height()
    ));

    Ok((image.width(), image.height()))
}

fn load_pdfium_bindings(
    resource_dir: Option<&Path>,
) -> Result<Box<dyn PdfiumLibraryBindings>, String> {
    // 1) Explicit override
    if let Ok(path) = std::env::var("PDFIUM_LIB_PATH") {
        if let Ok(b) = Pdfium::bind_to_library(&path) {
            thumb_log(&format!("pdfium: using PDFIUM_LIB_PATH={path}"));
            return Ok(b);
        }
        thumb_log(&format!(
            "pdfium: failed PDFIUM_LIB_PATH={path}, falling back"
        ));
    }

    // 2) Bundled paths (dev + packaged)
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(res) = resource_dir {
        #[cfg(target_os = "linux")]
        candidates.push(res.join("pdfium-linux-x64/lib/libpdfium.so"));
        #[cfg(target_os = "windows")]
        candidates.push(res.join("pdfium-win-x64/bin/pdfium.dll"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Common layouts: installed bundle keeps resources beside the exe; dev sits at target/{debug,release}
            #[cfg(target_os = "linux")]
            candidates.push(dir.join("libpdfium.so"));
            #[cfg(target_os = "linux")]
            candidates.push(dir.join("resources/pdfium-linux-x64/lib/libpdfium.so"));
            #[cfg(target_os = "windows")]
            candidates.push(dir.join("resources/pdfium-win-x64/bin/pdfium.dll"));

            // For dev builds where exe is target/{debug,release}/browsey.exe, project root is two levels up.
            let proj_root = dir.parent().and_then(|p| p.parent()).unwrap_or(dir);
            #[cfg(target_os = "linux")]
            candidates.push(proj_root.join("resources/pdfium-linux-x64/lib/libpdfium.so"));
            #[cfg(target_os = "windows")]
            candidates.push(proj_root.join("resources/pdfium-win-x64/bin/pdfium.dll"));

            // In case pdfium.dll is copied next to the exe (paranoia)
            #[cfg(target_os = "windows")]
            candidates.push(dir.join("pdfium.dll"));
        }
    }

    // 3) Common distro names/paths (fallback)
    #[cfg(target_os = "linux")]
    {
        candidates.extend([
            PathBuf::from("/usr/lib64/libpdfium.so"),
            PathBuf::from("/usr/lib/libpdfium.so"),
            PathBuf::from("/usr/lib64/libdeepin-pdfium.so.1"),
            PathBuf::from("/usr/lib64/libdeepin-pdfium.so"),
        ]);
    }

    for cand in candidates {
        if cand.exists() {
            let p = cand.to_string_lossy().to_string();
            if let Ok(b) = Pdfium::bind_to_library(&p) {
                thumb_log(&format!("pdfium: using candidate {}", p));
                return Ok(b);
            }
            thumb_log(&format!("pdfium: failed candidate {}, continuing", p));
        }
    }

    // 4) System search
    Pdfium::bind_to_system_library()
        .map_err(|e| format!("Pdfium load failed: {e}"))
        .map(|b| {
            thumb_log("pdfium: using system library search");
            b
        })
}

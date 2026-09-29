//! Shared, process-lifetime PDFium initialization for thumbnails and metadata.
//! pdfium-render 0.9.4 permits binding only once. Its `thread_safe` feature
//! serializes native calls; never rebind or destroy the library per document.

use once_cell::sync::OnceCell;
use pdfium_render::prelude::{Pdfium, PdfiumError};
use std::path::{Path, PathBuf};

static PDFIUM: OnceCell<Pdfium> = OnceCell::new();

pub(crate) fn pdfium(resource_dir: Option<&Path>) -> Result<&'static Pdfium, PdfiumError> {
    PDFIUM.get_or_try_init(|| {
        if let Ok(path) = std::env::var("PDFIUM_LIB_PATH") {
            if let Ok(bindings) = Pdfium::bind_to_library(&path) {
                tracing::debug!(path, "PDFium initialized from explicit override");
                return Ok(Pdfium::new(bindings));
            }
            tracing::debug!(path, "PDFium override unavailable; trying bundled library");
        }

        for path in candidates(resource_dir) {
            if path.is_file() {
                match Pdfium::bind_to_library(&path) {
                    Ok(bindings) => {
                        tracing::debug!(path = %path.display(), "PDFium initialized");
                        return Ok(Pdfium::new(bindings));
                    }
                    Err(error) => {
                        tracing::debug!(path = %path.display(), %error, "PDFium candidate unavailable");
                    }
                }
            }
        }

        // Failed initialization is not cached, so a later request can retry.
        Pdfium::bind_to_system_library().map(Pdfium::new)
    })
}

fn candidates(resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = resource_dir {
        bundled_paths(dir, &mut paths);
        // Tauri's resource directory can contain the whole resources/ tree.
        bundled_paths(&dir.join("resources"), &mut paths);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(Pdfium::pdfium_platform_library_name_at_path(dir));
            bundled_paths(&dir.join("resources"), &mut paths);
            // Source builds live in target/{debug,release}.
            if let Some(root) = dir.parent().and_then(Path::parent) {
                bundled_paths(&root.join("resources"), &mut paths);
            }
        }
    }
    #[cfg(target_os = "linux")]
    paths.extend([
        PathBuf::from("/usr/lib64/libpdfium.so"),
        PathBuf::from("/usr/lib/libpdfium.so"),
        PathBuf::from("/usr/lib64/libdeepin-pdfium.so.1"),
        PathBuf::from("/usr/lib64/libdeepin-pdfium.so"),
    ]);
    paths
}

fn bundled_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    #[cfg(target_os = "linux")]
    paths.push(dir.join("pdfium-linux-x64/lib/libpdfium.so"));
    #[cfg(target_os = "windows")]
    paths.push(dir.join("pdfium-win-x64/bin/pdfium.dll"));
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = (dir, paths);
}

#[cfg(all(test, any(target_os = "linux", target_os = "windows")))]
mod tests {
    use super::*;

    #[test]
    fn pdfium_searches_flat_and_nested_tauri_resource_layouts() {
        let dir = Path::new("bundle-root");
        let mut expected = Vec::new();
        bundled_paths(dir, &mut expected);
        bundled_paths(&dir.join("resources"), &mut expected);
        assert!(candidates(Some(dir)).starts_with(&expected));
    }
}

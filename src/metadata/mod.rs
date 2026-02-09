pub mod providers;
pub mod types;

use std::fs;
use std::path::Path;
use types::{ExtraMetadataResult, ExtraMetadataSection};

fn classify_kind(path: &Path, meta: &fs::Metadata) -> String {
    if meta.file_type().is_symlink() {
        return "symlink".to_string();
    }
    if meta.is_dir() {
        return "directory".to_string();
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "webp"
            | "svg"
            | "tiff"
            | "tif"
            | "tga"
            | "ico"
            | "avif"
    ) {
        return "image".to_string();
    }
    if ext == "pdf" {
        return "pdf".to_string();
    }
    if matches!(
        ext.as_str(),
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "wmv" | "m4v" | "mpeg" | "mpg"
    ) {
        return "video".to_string();
    }
    if matches!(
        ext.as_str(),
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma"
    ) {
        return "audio".to_string();
    }
    if matches!(
        ext.as_str(),
        "zip"
            | "tar"
            | "gz"
            | "bz2"
            | "xz"
            | "zst"
            | "tgz"
            | "tbz2"
            | "txz"
            | "tzst"
            | "7z"
            | "rar"
    ) {
        return "archive".to_string();
    }

    "generic".to_string()
}

pub fn collect_extra_metadata(path: &Path) -> Result<ExtraMetadataResult, String> {
    let meta =
        fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata for path: {e}"))?;
    let kind = classify_kind(path, &meta);

    let mut sections: Vec<ExtraMetadataSection> = Vec::new();

    match kind.as_str() {
        "image" => sections.extend(providers::image::collect(path)),
        "pdf" => sections.extend(providers::pdf::collect(path)),
        "video" => sections.extend(providers::video::collect(path)),
        "audio" => sections.extend(providers::audio::collect(path)),
        "archive" => sections.extend(providers::archive::collect(path)),
        _ => {}
    }

    sections.retain(|section| !section.is_empty());

    Ok(ExtraMetadataResult { kind, sections })
}

use std::fs::Metadata;
use std::path::Path;

pub fn icon_for(path: &Path, meta: &Metadata, is_link: bool) -> &'static str {
    if is_link {
        return "icons/scalable/mimetypes/inode-symlink.svg";
    }

    if meta.is_dir() {
        return "icons/scalable/places/folder.svg";
    }

    let name_lc = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Detect common multi-part archive extensions (e.g., .tar.gz)
    let is_tar_combo = name_lc.ends_with(".tar.gz")
        || name_lc.ends_with(".tar.bz2")
        || name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".tar.zst")
        || name_lc.ends_with(".tar.lz")
        || name_lc.ends_with(".tgz")
        || name_lc.ends_with(".tbz")
        || name_lc.ends_with(".tbz2")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tzst");

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        // Archives / compressed
        _ if is_tar_combo => "icons/scalable/mimetypes/package-x-generic.svg",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz" => {
            "icons/scalable/mimetypes/package-x-generic.svg"
        }
        // Executables / scripts
        "exe" | "bin" | "sh" | "bat" | "cmd" => "icons/scalable/mimetypes/application-x-executable.svg",
        "dll" | "so" | "dylib" => "icons/scalable/mimetypes/application-x-sharedlib.svg",
        // Code / text
        "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" | "tsx" | "jsx" | "java" | "go"
        | "rb" | "php" | "lua" | "json" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "md" | "txt" => {
            "icons/scalable/mimetypes/text-x-generic.svg"
        }
        // Media
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "tiff" => {
            "icons/scalable/mimetypes/image-x-generic.svg"
        }
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => "icons/scalable/mimetypes/audio-x-generic.svg",
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" => "icons/scalable/mimetypes/video-x-generic.svg",
        // Documents
        "pdf" => "icons/scalable/mimetypes/application-pdf.svg",
        "csv" | "xls" | "xlsx" => "icons/scalable/mimetypes/text-csv.svg",
        "ppt" | "pptx" => "icons/scalable/mimetypes/text-x-generic.svg",
        "doc" | "docx" | "odt" | "rtf" => "icons/scalable/mimetypes/text-x-generic.svg",
        _ => "icons/scalable/mimetypes/application-x-generic.svg",
    }
}

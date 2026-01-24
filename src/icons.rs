use std::fs::Metadata;
use std::path::Path;

pub fn icon_for(path: &Path, meta: &Metadata, is_link: bool) -> &'static str {
    if is_link {
        return "icons/scalable/browsey/shortcut.svg";
    }

    let name_lc = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if meta.is_dir() {
        let dir_icon = match name_lc.as_str() {
            "downloads" | "download" => Some("icons/scalable/browsey/download_folder.svg"),
            "documents" | "document" | "docs" => Some("icons/scalable/browsey/document_folder.svg"),
            "pictures" | "photos" | "images" => Some("icons/scalable/browsey/pictures_folder.svg"),
            "videos" | "video" | "movies" => Some("icons/scalable/browsey/video_folder.svg"),
            "music" | "audio" | "songs" => Some("icons/scalable/browsey/music_folder.svg"),
            "templates" => Some("icons/scalable/browsey/templates_folder.svg"),
            "public" | "publicshare" => Some("icons/scalable/browsey/public_folder.svg"),
            "desktop" => Some("icons/scalable/browsey/desktop_folder.svg"),
            "home" => Some("icons/scalable/browsey/home.svg"),
            _ => None,
        };
        return dir_icon.unwrap_or("icons/scalable/browsey/folder.svg");
    }

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
        _ if is_tar_combo => "icons/scalable/browsey/compressed.svg",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz" => {
            "icons/scalable/browsey/compressed.svg"
        }
        // Executables / scripts
        "exe" | "bin" | "sh" | "bat" | "cmd" => {
            "icons/scalable/mimetypes/application-x-executable.svg"
        }
        "dll" | "so" | "dylib" => "icons/scalable/mimetypes/application-x-sharedlib.svg",
        // Code / text
        "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" | "tsx" | "jsx" | "java" | "go"
        | "rb" | "php" | "lua" | "json" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "md"
        | "txt" => "icons/scalable/browsey/textfile.svg",
        // Media
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "tiff" => {
            "icons/scalable/browsey/picture_file.svg"
        }
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => {
            "icons/scalable/mimetypes/audio-x-generic.svg"
        }
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" => {
            "icons/scalable/mimetypes/video-x-generic.svg"
        }
        // Documents
        "pdf" => "icons/scalable/browsey/pdf_file.svg",
        "csv" | "xls" | "xlsx" | "xlsm" | "xlt" | "xltx" => {
            "icons/scalable/browsey/spreadsheet_file.svg"
        }
        "ppt" | "pptx" => "icons/scalable/browsey/presentation_file.svg",
        "doc" | "docx" | "docm" | "dot" | "dotx" => "icons/scalable/browsey/textfile.svg",
        "odt" | "rtf" => "icons/scalable/browsey/textfile.svg",
        _ => "icons/scalable/browsey/file.svg",
    }
}

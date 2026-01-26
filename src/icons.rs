use std::fs::Metadata;
use std::path::Path;

use mime::{APPLICATION, AUDIO, IMAGE, TEXT, VIDEO};
use mime_guess::MimeGuess;

// Browsey-specific icon mapping. Paths are relative to the packaged assets root
// (served from /icons/scalable/browsey/...).
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
        return dir_icon(&name_lc);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let mime = MimeGuess::from_path(path).first_raw();

    file_icon(&name_lc, &ext, mime)
}

fn dir_icon(name_lc: &str) -> &'static str {
    match name_lc {
        "downloads" | "download" => "icons/scalable/browsey/download_folder.svg",
        "documents" | "document" | "docs" => "icons/scalable/browsey/document_folder.svg",
        "pictures" | "photos" | "images" => "icons/scalable/browsey/pictures_folder.svg",
        "videos" | "video" | "movies" => "icons/scalable/browsey/video_folder.svg",
        "music" | "audio" | "songs" => "icons/scalable/browsey/music_folder.svg",
        "templates" => "icons/scalable/browsey/templates_folder.svg",
        "public" | "publicshare" => "icons/scalable/browsey/public_folder.svg",
        "desktop" => "icons/scalable/browsey/desktop_folder.svg",
        "home" => "icons/scalable/browsey/home.svg",
        _ => "icons/scalable/browsey/folder.svg",
    }
}

fn file_icon(name_lc: &str, ext: &str, mime: Option<&str>) -> &'static str {
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

    match ext {
        // Archives / compressed
        _ if is_tar_combo => "icons/scalable/browsey/compressed.svg",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz" | "tgz" | "tbz"
        | "tbz2" | "txz" | "tzst" => "icons/scalable/browsey/compressed.svg",
        // Executables / scripts
        "exe" | "bin" | "sh" | "bat" | "cmd" | "msi" => "icons/scalable/browsey/file.svg",
        "dll" | "so" | "dylib" => "icons/scalable/browsey/file.svg",
        // Code / text
        "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" | "tsx" | "jsx" | "java" | "go"
        | "rb" | "php" | "lua" | "json" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "md"
        | "txt" | "lock" => "icons/scalable/browsey/textfile.svg",
        // Media
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "tiff" | "avif" | "heic" => {
            "icons/scalable/browsey/picture_file.svg"
        }
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" => {
            "icons/scalable/browsey/file.svg"
        }
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" | "flv" | "m4v" => {
            "icons/scalable/browsey/video_file.svg"
        }
        // Documents
        "pdf" => "icons/scalable/browsey/pdf_file.svg",
        "xls" | "xlsx" | "xlsm" | "xlt" | "xltx" | "ods" | "csv" => {
            "icons/scalable/browsey/spreadsheet_file.svg"
        }
        "ppt" | "pptx" | "odp" => "icons/scalable/browsey/presentation_file.svg",
        "doc" | "docx" | "docm" | "dot" | "dotx" | "odt" | "rtf" => {
            "icons/scalable/browsey/textfile.svg"
        }
        _ => mime_icon(mime),
    }
}

fn mime_icon(mime: Option<&str>) -> &'static str {
    if let Some(raw) = mime {
        if let Ok(parsed) = raw.parse::<mime::Mime>() {
            let top = parsed.type_();
            if top == IMAGE {
                return "icons/scalable/browsey/picture_file.svg";
            }
            if top == AUDIO {
                return "icons/scalable/browsey/file.svg";
            }
            if top == VIDEO {
                return "icons/scalable/browsey/video_file.svg";
            }
            if top == TEXT {
                return "icons/scalable/browsey/textfile.svg";
            }
            if top == APPLICATION {
                let subtype = parsed.subtype();
                if subtype == "pdf" {
                    return "icons/scalable/browsey/pdf_file.svg";
                }
                if subtype == "zip"
                    || subtype == "x-7z-compressed"
                    || subtype == "x-rar-compressed"
                    || subtype == "x-xz"
                    || subtype == "x-bzip2"
                    || subtype == "x-tar"
                {
                    return "icons/scalable/browsey/compressed.svg";
                }
                if subtype == "x-executable" || subtype == "x-msdownload" {
                    return "icons/scalable/browsey/file.svg";
                }
            }
        }
    }

    "icons/scalable/browsey/file.svg"
}

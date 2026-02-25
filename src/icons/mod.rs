use std::fs::Metadata;
use std::path::Path;

use mime::{APPLICATION, AUDIO, IMAGE, TEXT, VIDEO};
use mime_guess::MimeGuess;

pub type IconId = u16;

// Keep the order in sync with frontend mapping.
pub mod icon_ids {
    use super::IconId;

    pub const SHORTCUT: IconId = 0;
    pub const DOWNLOAD_FOLDER: IconId = 1;
    pub const DOCUMENT_FOLDER: IconId = 2;
    pub const PICTURES_FOLDER: IconId = 3;
    pub const VIDEO_FOLDER: IconId = 4;
    pub const MUSIC_FOLDER: IconId = 5;
    pub const TEMPLATES_FOLDER: IconId = 6;
    pub const PUBLIC_FOLDER: IconId = 7;
    pub const DESKTOP_FOLDER: IconId = 8;
    pub const HOME_FOLDER: IconId = 9;
    pub const GENERIC_FOLDER: IconId = 10;
    pub const COMPRESSED: IconId = 11;
    pub const FILE: IconId = 12;
    pub const TEXTFILE: IconId = 13;
    pub const PICTURE_FILE: IconId = 14;
    pub const VIDEO_FILE: IconId = 15;
    pub const PDF_FILE: IconId = 16;
    pub const SPREADSHEET_FILE: IconId = 17;
    pub const PRESENTATION_FILE: IconId = 18;
    pub const AUDIO_FILE: IconId = 19;
    pub const EXECUTABLE_FILE: IconId = 20;
    pub const CLOUD: IconId = 21;
}

use icon_ids::*;

// Browsey-specific icon mapping. Icons are exposed as small numeric IDs for leaner payloads.
pub fn icon_id_for(path: &Path, meta: &Metadata, is_link: bool) -> IconId {
    if is_link {
        return SHORTCUT;
    }

    let name_lc = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if meta.is_dir() {
        return dir_icon_id(&name_lc);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let mime = MimeGuess::from_path(path).first_raw();

    file_icon_id(&name_lc, &ext, mime)
}

fn dir_icon_id(name_lc: &str) -> IconId {
    match name_lc {
        "downloads" | "download" => DOWNLOAD_FOLDER,
        "documents" | "document" | "docs" => DOCUMENT_FOLDER,
        "pictures" | "photos" | "images" => PICTURES_FOLDER,
        "videos" | "video" | "movies" => VIDEO_FOLDER,
        "music" | "audio" | "songs" => MUSIC_FOLDER,
        "templates" => TEMPLATES_FOLDER,
        "public" | "publicshare" => PUBLIC_FOLDER,
        "desktop" => DESKTOP_FOLDER,
        "home" => HOME_FOLDER,
        _ => GENERIC_FOLDER,
    }
}

fn file_icon_id(name_lc: &str, ext: &str, mime: Option<&str>) -> IconId {
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
        _ if is_tar_combo => COMPRESSED,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz" | "tgz" | "tbz"
        | "tbz2" | "txz" | "tzst" => COMPRESSED,
        // Executables / scripts
        "exe" | "bin" | "sh" | "bat" | "cmd" | "msi" => EXECUTABLE_FILE,
        "dll" | "so" | "dylib" => EXECUTABLE_FILE,
        // Code / text
        "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" | "tsx" | "jsx" | "java" | "go"
        | "rb" | "php" | "lua" | "json" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "md"
        | "txt" | "lock" => TEXTFILE,
        // Media
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "tiff" | "avif" | "heic" => {
            PICTURE_FILE
        }
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" => AUDIO_FILE,
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" | "flv" | "m4v" => VIDEO_FILE,
        // Documents
        "pdf" => PDF_FILE,
        "xls" | "xlsx" | "xlsm" | "xlt" | "xltx" | "ods" | "csv" => SPREADSHEET_FILE,
        "ppt" | "pptx" | "odp" => PRESENTATION_FILE,
        "doc" | "docx" | "docm" | "dot" | "dotx" | "odt" | "rtf" => TEXTFILE,
        _ => mime_icon_id(mime),
    }
}

fn mime_icon_id(mime: Option<&str>) -> IconId {
    if let Some(raw) = mime {
        if let Ok(parsed) = raw.parse::<mime::Mime>() {
            let top = parsed.type_();
            if top == IMAGE {
                return PICTURE_FILE;
            }
            if top == AUDIO {
                return AUDIO_FILE;
            }
            if top == VIDEO {
                return VIDEO_FILE;
            }
            if top == TEXT {
                return TEXTFILE;
            }
            if top == APPLICATION {
                let subtype = parsed.subtype();
                if subtype == "pdf" {
                    return PDF_FILE;
                }
                if subtype == "zip"
                    || subtype == "x-7z-compressed"
                    || subtype == "x-rar-compressed"
                    || subtype == "x-xz"
                    || subtype == "x-bzip2"
                    || subtype == "x-tar"
                {
                    return COMPRESSED;
                }
                if subtype == "x-executable" || subtype == "x-msdownload" {
                    return EXECUTABLE_FILE;
                }
            }
        }
    }

    FILE
}

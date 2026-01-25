use std::fs::{self, Metadata};
use std::path::Path;

pub fn icon_for(path: &Path, meta: &Metadata, is_link: bool) -> String {
    if let Some(sys_icon) = system_icon(path, meta, is_link) {
        return sys_icon;
    }

    if is_link {
        return "icons/scalable/browsey/shortcut.svg".into();
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
        return dir_icon.unwrap_or("icons/scalable/browsey/folder.svg").into();
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
            "icons/scalable/browsey/music_folder.svg"
        }
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" => {
            "icons/scalable/browsey/video_folder.svg"
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
    .into()
}

#[cfg(target_os = "linux")]
fn system_icon(path: &Path, meta: &Metadata, is_link: bool) -> Option<String> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;

    let mut names: Vec<String> = Vec::new();

    if is_link {
        names.push("emblem-symbolic-link".into());
        names.push("inode-symlink".into());
    }

    if meta.is_dir() {
        names.push("inode-directory".into());
        names.push("folder".into());
    } else {
        if let Some(mime) = mime_guess::from_path(path).first_raw() {
            names.push(mime.replace('/', "-"));
        }
        names.push("application-octet-stream".into());
    }

    let icon_bases = [
        "/usr/share/icons/Adwaita",
        "/usr/share/icons/hicolor",
        "/usr/share/icons",
        "/usr/share/pixmaps",
        "/usr/local/share/icons",
    ];

    let sizes = ["64x64", "48x48", "32x32", "scalable"];
    let categories = ["mimetypes", "places", "devices", "apps", "status"];
    let exts = ["png", "svg"];

    for base in icon_bases {
        for size in sizes {
            for cat in categories {
                for name in &names {
                    for ext in &exts {
                        let path = if size == "scalable" {
                            Path::new(base)
                                .join(size)
                                .join(cat)
                                .join(format!("{name}.{ext}"))
                        } else {
                            Path::new(base)
                                .join(size)
                                .join(cat)
                                .join(format!("{name}.{ext}"))
                        };

                        if let Ok(bytes) = fs::read(&path) {
                            let mime = if *ext == "svg" {
                                "image/svg+xml"
                            } else {
                                "image/png"
                            };
                            let data_url =
                                format!("data:{mime};base64,{}", B64.encode(bytes));
                            return Some(data_url);
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(not(target_os = "linux"))]
fn system_icon(_path: &Path, _meta: &Metadata, _is_link: bool) -> Option<String> {
    None
}

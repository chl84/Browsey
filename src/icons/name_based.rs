use std::path::Path;

use mime::{APPLICATION, AUDIO, IMAGE, MODEL, TEXT, VIDEO};

use super::{
    icon_ids::{
        AUDIO_FILE, CODE_FILE, COMPRESSED, DESKTOP_FOLDER, DISK_IMAGE_FILE, DOCUMENT_FILE,
        DOCUMENT_FOLDER, DOWNLOAD_FOLDER, EBOOK_FILE, EXECUTABLE_FILE, FILE, FONT_FILE,
        GENERIC_FOLDER, HOME_FOLDER, MODEL_3D_FILE, MUSIC_FOLDER, PACKAGE_FILE, PDF_FILE,
        PICTURES_FOLDER, PICTURE_FILE, PRESENTATION_FILE, PUBLIC_FOLDER, SPREADSHEET_FILE,
        TEMPLATES_FOLDER, TEXTFILE, VIDEO_FILE, VIDEO_FOLDER,
    },
    IconId,
};

pub(super) fn icon_id_for_name(name: &str, is_dir: bool, mime: Option<&str>) -> IconId {
    let name_lc = name.to_lowercase();
    if is_dir {
        return dir_icon_id(&name_lc);
    }

    let ext = Path::new(&name_lc)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    file_icon_id(&name_lc, ext, mime)
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
        // Treat RPM/DEB as installation packages before MIME fallback. The
        // extension-only MIME table also associates RPM with legacy audio.
        "rpm" | "deb" => PACKAGE_FILE,
        "iso" | "img" => DISK_IMAGE_FILE,
        "ttf" | "otf" | "ttc" | "woff" | "woff2" | "eot" => FONT_FILE,
        "epub" | "mobi" => EBOOK_FILE,
        // 3D models / scenes / CAD solids. Prefer their extension even when
        // a text-based model or packaged scene has a generic MIME type.
        "blend" | "blend1" | "blend2" | "obj" | "stl" | "fbx" | "gltf" | "glb" | "dae" | "ply"
        | "3ds" | "3mf" | "3dm" | "usd" | "usda" | "usdc" | "usdz" | "abc" | "step" | "stp"
        | "iges" | "igs" => MODEL_3D_FILE,
        // Source code
        "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" | "tsx" | "jsx" | "java" | "go"
        | "rb" | "php" | "lua" | "html" | "htm" | "css" => CODE_FILE,
        // Plain text and configuration
        "json" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "md" | "txt" | "lock" => TEXTFILE,
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
        "doc" | "docx" | "docm" | "dot" | "dotx" | "odt" | "rtf" => DOCUMENT_FILE,
        _ => mime_icon_id(mime),
    }
}

fn mime_icon_id(mime: Option<&str>) -> IconId {
    if let Some(raw) = mime {
        if let Ok(parsed) = raw.parse::<mime::Mime>() {
            let top = parsed.type_();
            if top == MODEL {
                return MODEL_3D_FILE;
            }
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

#[cfg(test)]
mod tests {
    use super::icon_id_for_name;
    use crate::icons::icon_ids::{
        COMPRESSED, FILE, GENERIC_FOLDER, MODEL_3D_FILE, PDF_FILE, PICTURES_FOLDER, TEXTFILE,
    };

    #[test]
    fn virtual_file_uses_extension_mapping() {
        assert_eq!(icon_id_for_name("report.pdf", false, None), PDF_FILE);
        assert_eq!(icon_id_for_name("archive.tar.gz", false, None), COMPRESSED);
        assert_eq!(icon_id_for_name("README", false, None), FILE);
    }

    #[test]
    fn virtual_directory_uses_named_folder_mapping() {
        assert_eq!(icon_id_for_name("Pictures", true, None), PICTURES_FOLDER);
        assert_eq!(icon_id_for_name("foo.bar", true, None), GENERIC_FOLDER);
    }

    #[test]
    fn model_extensions_use_the_3d_icon_without_changing_other_file_types() {
        for ext in [
            "blend", "blend1", "blend2", "obj", "stl", "fbx", "gltf", "glb", "dae", "ply", "3ds",
            "3mf", "3dm", "usd", "usda", "usdc", "usdz", "abc", "step", "stp", "iges", "igs",
        ] {
            for ext in [ext.to_string(), ext.to_uppercase()] {
                let name = format!("scene.{ext}");
                assert_eq!(
                    icon_id_for_name(&name, false, None),
                    MODEL_3D_FILE,
                    "{name}"
                );
                assert_eq!(
                    icon_id_for_name(&name, false, Some("text/plain")),
                    MODEL_3D_FILE,
                    "{name}"
                );
            }
        }
        assert_eq!(icon_id_for_name("folder.blend", true, None), GENERIC_FOLDER);
        assert_eq!(icon_id_for_name("notes.txt", false, None), TEXTFILE);
        assert_eq!(icon_id_for_name("compiled.o", false, None), FILE);
        assert_eq!(icon_id_for_name("unknown.xyz", false, None), FILE);
    }

    #[test]
    fn model_mime_type_handles_unrecognized_extensions() {
        assert_eq!(
            icon_id_for_name("scene.custom", false, Some("model/gltf+json")),
            MODEL_3D_FILE
        );
        assert_eq!(
            icon_id_for_name("scene.custom", false, Some("model/stl")),
            MODEL_3D_FILE
        );
        assert_eq!(
            icon_id_for_name("scene.custom", false, Some("invalid mime")),
            FILE
        );
    }
}

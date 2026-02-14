use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFileTypeMatch {
    pub label: &'static str,
    pub mime: &'static str,
    pub matched_ext: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct FileTypeDef {
    label: &'static str,
    mime: &'static str,
    extensions: &'static [&'static str],
}

const FILE_TYPE_DEFS: &[FileTypeDef] = &[
    FileTypeDef {
        label: "Plain Text",
        mime: "text/plain",
        extensions: &["txt", "text", "log"],
    },
    FileTypeDef {
        label: "Markdown",
        mime: "text/markdown",
        extensions: &["md", "markdown"],
    },
    FileTypeDef {
        label: "JSON",
        mime: "application/json",
        extensions: &["json"],
    },
    FileTypeDef {
        label: "JSON with Comments",
        mime: "application/json",
        extensions: &["jsonc"],
    },
    FileTypeDef {
        label: "YAML",
        mime: "application/x-yaml",
        extensions: &["yaml", "yml"],
    },
    FileTypeDef {
        label: "TOML",
        mime: "application/toml",
        extensions: &["toml"],
    },
    FileTypeDef {
        label: "INI Config",
        mime: "text/plain",
        extensions: &["ini", "cfg", "conf"],
    },
    FileTypeDef {
        label: "CSV",
        mime: "text/csv",
        extensions: &["csv"],
    },
    FileTypeDef {
        label: "TSV",
        mime: "text/tab-separated-values",
        extensions: &["tsv"],
    },
    FileTypeDef {
        label: "XML",
        mime: "application/xml",
        extensions: &["xml"],
    },
    FileTypeDef {
        label: "HTML",
        mime: "text/html",
        extensions: &["html", "htm", "xhtml"],
    },
    FileTypeDef {
        label: "CSS",
        mime: "text/css",
        extensions: &["css"],
    },
    FileTypeDef {
        label: "Sass",
        mime: "text/x-scss",
        extensions: &["scss", "sass"],
    },
    FileTypeDef {
        label: "Less",
        mime: "text/css",
        extensions: &["less"],
    },
    FileTypeDef {
        label: "JavaScript",
        mime: "text/javascript",
        extensions: &["js", "mjs", "cjs"],
    },
    FileTypeDef {
        label: "TypeScript",
        mime: "text/typescript",
        extensions: &["ts", "mts", "cts"],
    },
    FileTypeDef {
        label: "JSX",
        mime: "text/jsx",
        extensions: &["jsx"],
    },
    FileTypeDef {
        label: "TSX",
        mime: "text/tsx",
        extensions: &["tsx"],
    },
    FileTypeDef {
        label: "Svelte",
        mime: "text/plain",
        extensions: &["svelte"],
    },
    FileTypeDef {
        label: "Vue",
        mime: "text/plain",
        extensions: &["vue"],
    },
    FileTypeDef {
        label: "Rust",
        mime: "text/rust",
        extensions: &["rs"],
    },
    FileTypeDef {
        label: "Python",
        mime: "text/x-python",
        extensions: &["py"],
    },
    FileTypeDef {
        label: "Shell Script",
        mime: "text/x-shellscript",
        extensions: &["sh", "bash", "zsh", "fish"],
    },
    FileTypeDef {
        label: "C",
        mime: "text/x-c",
        extensions: &["c", "h"],
    },
    FileTypeDef {
        label: "C++",
        mime: "text/x-c++",
        extensions: &["cc", "cpp", "cxx", "hh", "hpp", "hxx"],
    },
    FileTypeDef {
        label: "Java",
        mime: "text/x-java",
        extensions: &["java"],
    },
    FileTypeDef {
        label: "Kotlin",
        mime: "text/x-kotlin",
        extensions: &["kt", "kts"],
    },
    FileTypeDef {
        label: "Go",
        mime: "text/x-go",
        extensions: &["go"],
    },
    FileTypeDef {
        label: "Ruby",
        mime: "text/x-ruby",
        extensions: &["rb"],
    },
    FileTypeDef {
        label: "PHP",
        mime: "text/x-php",
        extensions: &["php"],
    },
    FileTypeDef {
        label: "Swift",
        mime: "text/x-swift",
        extensions: &["swift"],
    },
    FileTypeDef {
        label: "Dart",
        mime: "text/x-dart",
        extensions: &["dart"],
    },
    FileTypeDef {
        label: "SQL",
        mime: "application/sql",
        extensions: &["sql"],
    },
    FileTypeDef {
        label: "PDF",
        mime: "application/pdf",
        extensions: &["pdf"],
    },
    FileTypeDef {
        label: "Rich Text",
        mime: "application/rtf",
        extensions: &["rtf"],
    },
    FileTypeDef {
        label: "Word Document",
        mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        extensions: &["docx"],
    },
    FileTypeDef {
        label: "Word Document",
        mime: "application/msword",
        extensions: &["doc"],
    },
    FileTypeDef {
        label: "Excel Spreadsheet",
        mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        extensions: &["xlsx"],
    },
    FileTypeDef {
        label: "Excel Spreadsheet",
        mime: "application/vnd.ms-excel",
        extensions: &["xls"],
    },
    FileTypeDef {
        label: "PowerPoint Presentation",
        mime: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        extensions: &["pptx"],
    },
    FileTypeDef {
        label: "PowerPoint Presentation",
        mime: "application/vnd.ms-powerpoint",
        extensions: &["ppt"],
    },
    FileTypeDef {
        label: "OpenDocument Text",
        mime: "application/vnd.oasis.opendocument.text",
        extensions: &["odt"],
    },
    FileTypeDef {
        label: "OpenDocument Spreadsheet",
        mime: "application/vnd.oasis.opendocument.spreadsheet",
        extensions: &["ods"],
    },
    FileTypeDef {
        label: "OpenDocument Presentation",
        mime: "application/vnd.oasis.opendocument.presentation",
        extensions: &["odp"],
    },
    FileTypeDef {
        label: "PNG Image",
        mime: "image/png",
        extensions: &["png"],
    },
    FileTypeDef {
        label: "JPEG Image",
        mime: "image/jpeg",
        extensions: &["jpg", "jpeg"],
    },
    FileTypeDef {
        label: "GIF Image",
        mime: "image/gif",
        extensions: &["gif"],
    },
    FileTypeDef {
        label: "WebP Image",
        mime: "image/webp",
        extensions: &["webp"],
    },
    FileTypeDef {
        label: "AVIF Image",
        mime: "image/avif",
        extensions: &["avif"],
    },
    FileTypeDef {
        label: "SVG Image",
        mime: "image/svg+xml",
        extensions: &["svg"],
    },
    FileTypeDef {
        label: "Bitmap Image",
        mime: "image/bmp",
        extensions: &["bmp"],
    },
    FileTypeDef {
        label: "TIFF Image",
        mime: "image/tiff",
        extensions: &["tif", "tiff"],
    },
    FileTypeDef {
        label: "Icon",
        mime: "image/x-icon",
        extensions: &["ico"],
    },
    FileTypeDef {
        label: "MP3 Audio",
        mime: "audio/mpeg",
        extensions: &["mp3"],
    },
    FileTypeDef {
        label: "WAV Audio",
        mime: "audio/wav",
        extensions: &["wav"],
    },
    FileTypeDef {
        label: "FLAC Audio",
        mime: "audio/flac",
        extensions: &["flac"],
    },
    FileTypeDef {
        label: "Ogg Audio",
        mime: "audio/ogg",
        extensions: &["ogg"],
    },
    FileTypeDef {
        label: "AAC Audio",
        mime: "audio/aac",
        extensions: &["aac"],
    },
    FileTypeDef {
        label: "M4A Audio",
        mime: "audio/mp4",
        extensions: &["m4a"],
    },
    FileTypeDef {
        label: "MP4 Video",
        mime: "video/mp4",
        extensions: &["mp4", "m4v"],
    },
    FileTypeDef {
        label: "Matroska Video",
        mime: "video/x-matroska",
        extensions: &["mkv"],
    },
    FileTypeDef {
        label: "QuickTime Video",
        mime: "video/quicktime",
        extensions: &["mov"],
    },
    FileTypeDef {
        label: "AVI Video",
        mime: "video/x-msvideo",
        extensions: &["avi"],
    },
    FileTypeDef {
        label: "WebM Video",
        mime: "video/webm",
        extensions: &["webm"],
    },
    FileTypeDef {
        label: "ZIP Archive",
        mime: "application/zip",
        extensions: &["zip"],
    },
    FileTypeDef {
        label: "7z Archive",
        mime: "application/x-7z-compressed",
        extensions: &["7z"],
    },
    FileTypeDef {
        label: "RAR Archive",
        mime: "application/vnd.rar",
        extensions: &["rar"],
    },
    FileTypeDef {
        label: "TAR Archive",
        mime: "application/x-tar",
        extensions: &["tar"],
    },
    FileTypeDef {
        label: "Gzip Archive",
        mime: "application/gzip",
        extensions: &["gz"],
    },
    FileTypeDef {
        label: "Bzip2 Archive",
        mime: "application/x-bzip2",
        extensions: &["bz2"],
    },
    FileTypeDef {
        label: "XZ Archive",
        mime: "application/x-xz",
        extensions: &["xz"],
    },
    FileTypeDef {
        label: "Zstd Archive",
        mime: "application/zstd",
        extensions: &["zst"],
    },
    FileTypeDef {
        label: "Compressed TAR Archive",
        mime: "application/gzip",
        extensions: &["tar.gz", "tgz"],
    },
    FileTypeDef {
        label: "Compressed TAR Archive",
        mime: "application/x-bzip2",
        extensions: &["tar.bz2", "tbz2"],
    },
    FileTypeDef {
        label: "Compressed TAR Archive",
        mime: "application/x-xz",
        extensions: &["tar.xz", "txz"],
    },
    FileTypeDef {
        label: "Compressed TAR Archive",
        mime: "application/zstd",
        extensions: &["tar.zst"],
    },
];

const NAME_TYPES: &[(&str, &str, &str)] = &[
    ("dockerfile", "Dockerfile", "text/plain"),
    ("makefile", "Makefile", "text/x-makefile"),
    ("cmakelists.txt", "CMake", "text/plain"),
    ("license", "License Text", "text/plain"),
    ("readme", "Readme", "text/plain"),
    (".gitignore", "Git Ignore", "text/plain"),
    (".gitattributes", "Git Attributes", "text/plain"),
    (".gitmodules", "Git Submodules", "text/plain"),
    (".editorconfig", "EditorConfig", "text/plain"),
    (".env", "Environment File", "text/plain"),
];

const COMPOUND_EXTENSIONS: &[&str] = &["tar.zst", "tar.bz2", "tar.gz", "tar.xz"];

fn extract_extension(normalized_name: &str) -> Option<&str> {
    if normalized_name.is_empty() || normalized_name.ends_with('.') {
        return None;
    }

    for compound in COMPOUND_EXTENSIONS {
        let needed = compound.len() + 1;
        if normalized_name.len() <= needed {
            continue;
        }
        if !normalized_name.ends_with(compound) {
            continue;
        }
        let split_idx = normalized_name.len() - needed;
        if normalized_name.as_bytes().get(split_idx) == Some(&b'.') {
            return Some(compound);
        }
    }

    let (stem, ext) = normalized_name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext)
}

#[tauri::command]
pub fn detect_new_file_type(name: String) -> Option<NewFileTypeMatch> {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    for (known_name, label, mime) in NAME_TYPES {
        if normalized == *known_name {
            return Some(NewFileTypeMatch {
                label,
                mime,
                matched_ext: None,
            });
        }
    }

    let ext = extract_extension(&normalized)?;
    for def in FILE_TYPE_DEFS {
        for known_ext in def.extensions {
            if ext == *known_ext {
                return Some(NewFileTypeMatch {
                    label: def.label,
                    mime: def.mime,
                    matched_ext: Some(known_ext),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::detect_new_file_type;

    #[test]
    fn detects_regular_extension() {
        let found = detect_new_file_type("notes.md".to_string()).expect("should resolve type");
        assert_eq!(found.label, "Markdown");
        assert_eq!(found.mime, "text/markdown");
        assert_eq!(found.matched_ext, Some("md"));
    }

    #[test]
    fn detects_compound_extension() {
        let found = detect_new_file_type("backup.tar.gz".to_string()).expect("should resolve type");
        assert_eq!(found.label, "Compressed TAR Archive");
        assert_eq!(found.matched_ext, Some("tar.gz"));
    }

    #[test]
    fn detects_special_name() {
        let found = detect_new_file_type("Dockerfile".to_string()).expect("should resolve type");
        assert_eq!(found.label, "Dockerfile");
        assert_eq!(found.matched_ext, None);
    }

    #[test]
    fn ignores_unknown_extension() {
        assert!(detect_new_file_type("archive.weirdext".to_string()).is_none());
    }
}

use std::fs::Metadata;
use std::path::Path;

use mime_guess::MimeGuess;

mod name_based;

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
    pub const MODEL_3D_FILE: IconId = 22;
    pub const NETWORK_FOLDER: IconId = 23;
    pub const DOCUMENT_FILE: IconId = 24;
    pub const CODE_FILE: IconId = 25;
    pub const PACKAGE_FILE: IconId = 26;
    pub const DISK_IMAGE_FILE: IconId = 27;
    pub const FONT_FILE: IconId = 28;
    pub const EBOOK_FILE: IconId = 29;
    pub const CONFIG_FILE: IconId = 30;
    pub const DATABASE_FILE: IconId = 31;
    pub const VECTOR_FILE: IconId = 32;
}

use icon_ids::SHORTCUT;
use name_based::icon_id_for_name;

// Browsey-specific icon mapping. Icons are exposed as small numeric IDs for leaner payloads.
pub fn icon_id_for(path: &Path, meta: &Metadata, is_link: bool) -> IconId {
    if is_link {
        return SHORTCUT;
    }

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let is_dir = meta.is_dir();
    let mime = MimeGuess::from_path(path).first_raw();
    icon_id_for_name(name, is_dir, mime)
}

// Resolve icon for non-filesystem entries (for example cloud listing rows).
pub fn icon_id_for_virtual_entry(name: &str, is_dir: bool) -> IconId {
    icon_id_for_name(name, is_dir, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_icon_assets_share_the_generic_document_and_match_the_frontend_id() {
        let resource = include_str!("../../resources/icons/scalable/model_3d_file.svg");
        let public = include_str!("../../frontend/public/icons/scalable/browsey/model_3d_file.svg");
        assert_eq!(resource, public);
        for shape in ["M28 14H76L100 38V114H28V14Z", "M76 14v24h24"] {
            assert!(include_str!("../../resources/icons/scalable/file.svg").contains(shape));
            assert!(resource.contains(shape));
        }
        let frontend_icons = include_str!("../../frontend/src/features/explorer/helpers/icons.ts");
        let paths: Vec<_> = frontend_icons
            .lines()
            .filter(|line| line.trim().starts_with("'icons/scalable/browsey/"))
            .collect();
        assert!(paths[usize::from(icon_ids::MODEL_3D_FILE)].contains("model_3d_file.svg"));
    }

    #[test]
    fn local_and_virtual_file_types_share_icons_while_links_keep_the_shortcut() {
        // Only metadata is needed; no file contents or real user files are read.
        let meta = std::fs::metadata(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .expect("project file metadata");
        for (name, expected) in [
            ("mesh.STL", icon_ids::MODEL_3D_FILE),
            ("scene.blend", icon_ids::MODEL_3D_FILE),
            ("part.step", icon_ids::MODEL_3D_FILE),
            ("report.DOCX", icon_ids::DOCUMENT_FILE),
            ("report.odt", icon_ids::DOCUMENT_FILE),
            ("report.rtf", icon_ids::DOCUMENT_FILE),
            ("main.RS", icon_ids::CODE_FILE),
            ("main.py", icon_ids::CODE_FILE),
            ("main.js", icon_ids::CODE_FILE),
            ("main.ts", icon_ids::CODE_FILE),
            ("package.rpm", icon_ids::PACKAGE_FILE),
            ("package.RPM", icon_ids::PACKAGE_FILE),
            ("package.deb", icon_ids::PACKAGE_FILE),
            ("image.ISO", icon_ids::DISK_IMAGE_FILE),
            ("image.img", icon_ids::DISK_IMAGE_FILE),
            ("typeface.TTF", icon_ids::FONT_FILE),
            ("typeface.otf", icon_ids::FONT_FILE),
            ("typeface.woff", icon_ids::FONT_FILE),
            ("typeface.woff2", icon_ids::FONT_FILE),
            ("book.EPUB", icon_ids::EBOOK_FILE),
            ("book.mobi", icon_ids::EBOOK_FILE),
            ("notes.txt", icon_ids::TEXTFILE),
            ("config.TOML", icon_ids::CONFIG_FILE),
            ("config.yaml", icon_ids::CONFIG_FILE),
            ("config.yml", icon_ids::CONFIG_FILE),
            ("config.ini", icon_ids::CONFIG_FILE),
            ("config.cfg", icon_ids::CONFIG_FILE),
            ("config.conf", icon_ids::CONFIG_FILE),
            ("config.json", icon_ids::CONFIG_FILE),
            ("database.SQLITE", icon_ids::DATABASE_FILE),
            ("database.sqlite3", icon_ids::DATABASE_FILE),
            ("database.db", icon_ids::DATABASE_FILE),
            ("drawing.SVG", icon_ids::VECTOR_FILE),
            ("drawing.svgz", icon_ids::VECTOR_FILE),
            ("drawing.ai", icon_ids::VECTOR_FILE),
            ("drawing.eps", icon_ids::VECTOR_FILE),
            ("photo.png", icon_ids::PICTURE_FILE),
            ("report.pdf", icon_ids::PDF_FILE),
            ("run.sh", icon_ids::EXECUTABLE_FILE),
            ("program.bin", icon_ids::EXECUTABLE_FILE),
            ("song.mp3", icon_ids::AUDIO_FILE),
            ("archive.tar.gz", icon_ids::COMPRESSED),
            ("unknown.custom", icon_ids::FILE),
        ] {
            assert_eq!(
                icon_id_for(Path::new(name), &meta, false),
                expected,
                "{name}"
            );
            assert_eq!(icon_id_for_virtual_entry(name, false), expected, "{name}");
            assert_eq!(
                icon_id_for_virtual_entry(name, true),
                icon_ids::GENERIC_FOLDER,
                "directory named {name}"
            );
            assert_eq!(
                icon_id_for(Path::new(name), &meta, true),
                icon_ids::SHORTCUT,
                "link named {name}"
            );
        }
    }
}

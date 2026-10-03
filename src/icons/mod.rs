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
    fn local_and_virtual_models_share_the_icon_while_links_keep_the_shortcut() {
        // Only metadata is needed; no model contents or real user files are read.
        let meta = std::fs::metadata(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .expect("project file metadata");
        for name in ["mesh.STL", "scene.blend", "part.step"] {
            assert_eq!(
                icon_id_for(Path::new(name), &meta, false),
                icon_ids::MODEL_3D_FILE
            );
            assert_eq!(
                icon_id_for_virtual_entry(name, false),
                icon_ids::MODEL_3D_FILE
            );
            assert_eq!(
                icon_id_for(Path::new(name), &meta, true),
                icon_ids::SHORTCUT
            );
        }
    }
}

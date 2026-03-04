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

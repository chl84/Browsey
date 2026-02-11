use once_cell::sync::Lazy;
use resvg::usvg::{fontdb, Options};
use std::{path::Path, sync::Arc};

static SVG_FONT_DB: Lazy<Arc<fontdb::Database>> = Lazy::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    Arc::new(db)
});

pub fn usvg_options_for_path(path: &Path) -> Options<'static> {
    let mut options = Options::default();
    options.resources_dir = path.parent().map(|p| p.to_path_buf());
    options.fontdb = SVG_FONT_DB.clone();
    options
}

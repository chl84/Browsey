use once_cell::sync::Lazy;
use resvg::usvg::{fontdb, Options};
use std::{path::Path, sync::Arc};

static SVG_FONT_DB: Lazy<Arc<fontdb::Database>> = Lazy::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    Arc::new(db)
});

pub fn usvg_options_for_path(path: &Path) -> Options<'static> {
    Options {
        resources_dir: path.parent().map(|p| p.to_path_buf()),
        fontdb: SVG_FONT_DB.clone(),
        ..Options::default()
    }
}

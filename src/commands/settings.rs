//! Persisted UI settings such as column widths.

use crate::db::{load_column_widths, save_column_widths};

#[tauri::command]
pub fn store_column_widths(widths: Vec<f64>) -> Result<(), String> {
    let conn = crate::db::open()?;
    save_column_widths(&conn, &widths)
}

#[tauri::command]
pub fn load_saved_column_widths() -> Result<Option<Vec<f64>>, String> {
    let conn = crate::db::open()?;
    load_column_widths(&conn)
}

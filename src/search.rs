use crate::entry::{build_entry, FsEntry};
use std::fs::{self, ReadDir};
use std::path::PathBuf;

pub fn search_recursive(root: PathBuf, query: String) -> Result<Vec<FsEntry>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let mut stack = vec![root];
    let needle = query.to_lowercase();

    while let Some(dir) = stack.pop() {
        let iter: ReadDir = match fs::read_dir(&dir) {
            Ok(i) => i,
            Err(_) => continue,
        };

        for entry in iter {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let is_dir = file_type.is_dir();
            let is_link = file_type.is_symlink();
            let name_lc = entry.file_name().to_string_lossy().to_lowercase();

            if name_lc.contains(&needle) {
                let meta = match fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                results.push(build_entry(&path, &meta, is_link, false));
            }

            // Only traverse real directories (not symlinks) to avoid loops.
            if is_dir && !is_link {
                stack.push(path);
            }
        }
    }

    Ok(results)
}

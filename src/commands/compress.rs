use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use crate::fs_utils::{check_no_symlink_components, sanitize_path_follow};

fn ensure_same_parent(paths: &[PathBuf]) -> Result<PathBuf, String> {
    let mut parent: Option<PathBuf> = None;
    for p in paths {
        match p.parent() {
            Some(par) => match parent {
                Some(ref prev) if prev != par => {
                    return Err("All items must be in the same folder to compress together".into())
                }
                Some(_) => {}
                None => parent = Some(par.to_path_buf()),
            },
            None => return Err("Cannot compress filesystem root".into()),
        }
    }
    parent.ok_or_else(|| "Missing parent for paths".into())
}

fn safe_name(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Name cannot be empty".into());
    }
    if name.contains(['/', '\\']) {
        return Err("Name cannot contain path separators".into());
    }
    Ok(name.to_string())
}

fn destination_path(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let mut base = safe_name(name)?;
    if !base.to_lowercase().ends_with(".zip") {
        base.push_str(".zip");
    }
    let mut candidate = parent.join(&base);
    let mut idx = 1;
    while candidate.exists() {
        candidate = parent.join(format!("{} ({idx}).zip", base.trim_end_matches(".zip")));
        idx += 1;
    }
    Ok(candidate)
}

fn add_path_to_zip(
    zip: &mut ZipWriter<BufWriter<File>>,
    base: &Path,
    path: &Path,
    level: u32,
    options: FileOptions,
) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Symlinks are not supported for compression".into());
    }

    let rel = path
        .strip_prefix(base)
        .map_err(|_| "Paths must share the same parent")?;
    let mut rel_name = rel.to_string_lossy().replace('\\', "/");
    let is_dir = meta.is_dir();
    if is_dir && !rel_name.ends_with('/') {
        rel_name.push('/');
    }

    let method = if level == 0 {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let opts = options
        .clone()
        .compression_method(method)
        .compression_level(Some(level as i32));

    if is_dir {
        zip.add_directory(rel_name, opts)
            .map_err(|e| format!("Failed to add directory to zip: {e}"))?;
        return Ok(());
    }

    zip.start_file(rel_name, opts)
        .map_err(|e| format!("Failed to start zip entry: {e}"))?;
    let mut f = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    std::io::copy(&mut f, zip).map_err(|e| format!("Failed to write file to zip: {e}"))?;
    Ok(())
}

fn collect_paths(input: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for path in input {
        let meta =
            fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Symlinks are not supported for compression".into());
        }
        if meta.is_dir() {
            for entry in WalkDir::new(path) {
                let entry = entry.map_err(|e| format!("Failed to read directory: {e}"))?;
                let p = entry.into_path();
                let meta = fs::symlink_metadata(&p)
                    .map_err(|e| format!("Failed to read metadata: {e}"))?;
                if meta.file_type().is_symlink() {
                    return Err("Symlinks are not supported for compression".into());
                }
                out.push(p);
            }
        } else {
            out.push(path.clone());
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn compress_entries(
    paths: Vec<String>,
    name: Option<String>,
    level: Option<u32>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("Nothing to compress".into());
    }
    let mut resolved: Vec<PathBuf> = Vec::new();
    for raw in paths {
        let pb = sanitize_path_follow(&raw, true)?;
        check_no_symlink_components(&pb)?;
        resolved.push(pb);
    }
    let parent = ensure_same_parent(&resolved)?;

    let suggested = if resolved.len() == 1 {
        resolved
            .get(0)
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("archive")
            .to_string()
    } else {
        "Archive".to_string()
    };
    let dest_name = name.unwrap_or(suggested);
    let lvl = level.unwrap_or(6).min(9);
    let mut dest = destination_path(&parent, &dest_name)?;
    let file = loop {
        match File::options().write(true).create_new(true).open(&dest) {
            Ok(f) => break f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                dest = destination_path(&parent, &dest_name)?;
                continue;
            }
            Err(e) => return Err(format!("Failed to create destination: {e}")),
        }
    };

    let mut writer = ZipWriter::new(BufWriter::new(file));
    let opts = FileOptions::default();

    let all = collect_paths(&resolved)?;
    for path in all {
        add_path_to_zip(&mut writer, &parent, &path, lvl, opts.clone())?;
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;

    Ok(dest.to_string_lossy().into_owned())
}

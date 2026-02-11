use std::{
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

use rar_stream::{
    InnerFile as RarInnerFile, LocalFileMedia as RarLocalFileMedia,
    ParseOptions as RarParseOptions, RarFilesPackage, ReadInterval as RarReadInterval,
};
use tauri::async_runtime;

use super::util::{
    check_cancel, clean_relative_path, first_component, map_copy_err, open_unique_file,
    CreatedPaths, ExtractBudget, ProgressEmitter, SkipStats, CHUNK,
};

pub(super) fn single_root_in_rar(path: &Path) -> Result<Option<PathBuf>, String> {
    let entries = parse_rar_entries(path)?;
    let mut root: Option<PathBuf> = None;
    for entry in entries {
        let raw_name = entry.name.replace('\\', "/");
        let raw_path = PathBuf::from(raw_name.clone());
        let clean_rel = match clean_relative_path(&raw_path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }
        let Some(first) = first_component(&clean_rel) else {
            continue;
        };
        let rest_is_empty = clean_rel.components().count() == 1;
        let is_dir = raw_name.ends_with('/')
            || raw_name.ends_with('\\')
            || (entry.length == 0 && rest_is_empty);
        if !is_dir && rest_is_empty {
            return Ok(None);
        }
        match &root {
            Some(r) if r != &first => return Ok(None),
            None => root = Some(first),
            _ => {}
        }
    }
    Ok(root)
}

pub(super) fn extract_rar(
    entries: Vec<RarInnerFile>,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
) -> Result<(), String> {
    for entry in entries {
        check_cancel(cancel).map_err(|e| map_copy_err("Extraction cancelled", e))?;
        let raw_name = entry.name.clone();
        let normalized = raw_name.replace('\\', "/");
        let raw_path = Path::new(&normalized).to_path_buf();

        // rar-stream mangler komplett dekoder for komprimerte entries; avbryt heller enn å skrive korrupte data.
        if entry.is_compressed() {
            return Err(format!(
                "RAR entry uses unsupported compression method: {raw_name}"
            ));
        }

        let clean_rel = match clean_relative_path(&raw_path) {
            Ok(p) => p,
            Err(err) => {
                stats.skip_unsupported(&raw_name, &err);
                continue;
            }
        };
        let clean_rel = if let Some(prefix) = strip_prefix {
            match clean_rel.strip_prefix(prefix) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => clean_rel,
            }
        } else {
            clean_rel
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }
        let dest_path = dest_dir.join(clean_rel);
        let is_dir = normalized.ends_with('/') || normalized.ends_with('\\');

        if is_dir {
            if !dest_path.exists() {
                if let Err(e) = fs::create_dir_all(&dest_path) {
                    stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
                } else {
                    created.record_dir(dest_path.clone());
                }
            } else if let Err(e) = fs::create_dir_all(&dest_path) {
                stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
            }
            continue;
        }

        if dest_path.exists() {
            if let Some(p) = progress {
                p.add(entry.length.max(1));
            }
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                    continue;
                }
                created.record_dir(parent.to_path_buf());
            } else if let Err(e) = fs::create_dir_all(parent) {
                stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                continue;
            }
        }

        let (file, dest_actual) =
            open_unique_file(&dest_path).map_err(|e| format!("Failed to create file: {e}"))?;
        created.record_file(dest_actual.clone());
        let mut out = BufWriter::with_capacity(CHUNK, file);
        write_rar_entry_streaming(&entry, &raw_name, &mut out, progress, cancel, budget)?;
    }
    Ok(())
}

fn write_rar_entry_streaming(
    entry: &RarInnerFile,
    raw_name: &str,
    out: &mut BufWriter<std::fs::File>,
    progress: Option<&ProgressEmitter>,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
) -> Result<(), String> {
    let mut start = 0u64;
    let chunk_len = CHUNK as u64;

    while start < entry.length {
        check_cancel(cancel).map_err(|e| map_copy_err("Extraction cancelled", e))?;
        let end = (start.saturating_add(chunk_len).saturating_sub(1)).min(entry.length - 1);
        let data = async_runtime::block_on(entry.read_range(RarReadInterval { start, end }))
            .map_err(|e| format!("Failed to read rar entry {raw_name}: {e}"))?;
        if data.is_empty() {
            return Err(format!("Failed to read rar entry {raw_name}: empty chunk"));
        }

        budget
            .reserve_bytes(data.len() as u64)
            .map_err(|e| map_copy_err("Extraction size cap exceeded", e))?;
        out.write_all(&data)
            .map_err(|e| map_copy_err(&format!("Failed to write {raw_name}"), e))?;
        if let Some(p) = progress {
            p.add(data.len() as u64);
        }
        start = start.saturating_add(data.len() as u64);
    }

    out.flush()
        .map_err(|e| map_copy_err(&format!("Failed to flush {raw_name}"), e))
}

pub(super) fn parse_rar_entries(path: &Path) -> Result<Vec<RarInnerFile>, String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| "Archive path is not valid UTF-8".to_string())?;
    let media = Arc::new(
        RarLocalFileMedia::new(path_str).map_err(|e| format!("Failed to open rar archive: {e}"))?,
    );
    let package = RarFilesPackage::new(vec![media]);
    async_runtime::block_on(async move {
        package
            .parse(RarParseOptions::default())
            .await
            .map_err(|e| format!("Failed to read rar: {e}"))
    })
}

pub(super) fn rar_uncompressed_total_from_entries(entries: &[RarInnerFile]) -> Result<u64, String> {
    let mut total = 0u64;
    for entry in entries {
        total = total.saturating_add(entry.length);
    }
    Ok(total)
}

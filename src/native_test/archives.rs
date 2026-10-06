//! Bounded archive operations on generated inputs; ordinary sessions stay denied.
use super::{check_path, no_links, workspaces};
use serde_json::Value;
use std::{fs, path::Path};

fn inspect(path: &Path, entries: &mut usize, depth: usize) -> Result<(), &'static str> {
    if depth > 8 || *entries == 0 {
        return Err("Generated archive tree exceeds its bound");
    }
    *entries -= 1;
    no_links(path)?;
    let meta = fs::symlink_metadata(path).map_err(|_| "Missing generated archive input")?;
    if meta.is_file() {
        let _ = workspaces::private_file(path)?;
    } else if meta.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| "Cannot inspect generated archive tree")? {
            inspect(
                &entry.map_err(|_| "Cannot inspect archive entry")?.path(),
                entries,
                depth + 1,
            )?;
        }
    } else {
        return Err("Archive inputs must be regular files/directories");
    }
    Ok(())
}

pub(super) fn authorize(
    roots: &[String],
    mode: Option<&str>,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    if !matches!(
        command,
        "can_extract_paths"
            | "compress_entries"
            | "compress_cloud_entries"
            | "extract_archive"
            | "extract_archives"
            | "extract_cloud_archive"
    ) {
        return None;
    }
    Some((|| {
        if mode != Some("archives") {
            return Err("Archives require the isolated archive mode");
        }
        let object = body
            .as_object()
            .ok_or("Archive arguments must be an object")?;
        let single = matches!(command, "extract_archive" | "extract_cloud_archive");
        let compress = command.starts_with("compress_");
        if object.keys().any(|k| {
            !matches!(
                k.as_str(),
                "path" | "paths" | "name" | "level" | "password" | "progressEvent"
            )
        }) {
            return Err("Unexpected archive argument");
        }
        let paths: Vec<&str> = if single {
            vec![object
                .get("path")
                .and_then(Value::as_str)
                .ok_or("Explicit archive path required")?]
        } else {
            object
                .get("paths")
                .and_then(Value::as_array)
                .ok_or("Explicit archive paths required")?
                .iter()
                .map(|p| p.as_str().ok_or("Invalid archive path"))
                .collect::<Result<_, _>>()?
        };
        if paths.is_empty() || paths.len() > 16 {
            return Err("Bounded nonempty archive inputs required");
        }
        if compress {
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Explicit archive name required")?;
            if name.is_empty()
                || name.len() > 64
                || name.contains(['/', '\\', '\0'])
                || matches!(name, "." | "..")
            {
                return Err("Archive name must be a bounded leaf");
            }
        }
        if object.get("password").is_some_and(|p| {
            !p.is_null() && p.as_str().is_none_or(|p| p.len() > 128 || p.contains('\0'))
        }) {
            return Err("Invalid bounded archive password");
        }
        // Authorize all lexical arguments before inspecting any input.
        for path in &paths {
            check_path(roots, path)?;
            let cloud = path.starts_with("rclone://");
            if command != "can_extract_paths" && cloud != command.contains("cloud") {
                return Err("Wrong archive provider");
            }
            if roots.iter().any(|root| root == path) {
                return Err("Archive operations cannot target the run root");
            }
        }
        let mut remaining = 128;
        for path in paths {
            if !path.starts_with("rclone://") {
                inspect(Path::new(path), &mut remaining, 0)?;
            }
        }
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn mode_and_all_lexical_arguments_are_checked_before_io() {
        let roots = vec![
            "/unavailable/ai_agent_testfolder/.bnt-00000000000040008000000000000000/files".into(),
        ];
        for (mode, command, body) in [
            (
                None,
                "extract_archive",
                json!({"path":format!("{}/a.zip",roots[0])}),
            ),
            (
                Some("archives"),
                "extract_archive",
                json!({"path":"/personal/a.zip"}),
            ),
            (Some("archives"), "extract_archive", json!({})),
            (
                Some("archives"),
                "compress_entries",
                json!({"paths":[format!("{}/a",roots[0])],"name":"../outside.zip"}),
            ),
            (
                Some("archives"),
                "extract_cloud_archive",
                json!({"path":format!("{}/a.zip",roots[0])}),
            ),
        ] {
            assert!(authorize(&roots, mode, command, &body).unwrap().is_err());
        }
    }
}

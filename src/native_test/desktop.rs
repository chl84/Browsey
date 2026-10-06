//! Separately approved native desktop services, after actual namespace proof.
use super::{check_path, collect_paths, no_links, workspaces};
use serde_json::Value;
use std::{collections::BTreeMap, fs, io::Read, path::Path};

#[cfg(feature = "native-test")]
pub(super) fn validate_isolation(
    mode: Option<&str>,
    run_id: &str,
    run: &Path,
) -> Result<(), &'static str> {
    let Some(mode) = mode else {
        return Ok(());
    };
    if mode != "desktop-services" {
        return Err("Unknown approved desktop mode");
    }
    let raw =
        std::env::var("BROWSEY_NATIVE_ISOLATED").map_err(|_| "Missing desktop namespace origin")?;
    let origin: Value =
        serde_json::from_str(&raw).map_err(|_| "Invalid desktop namespace origin")?;
    let mount =
        fs::read_link("/proc/self/ns/mnt").map_err(|_| "Cannot inspect desktop mount namespace")?;
    let pid =
        fs::read_link("/proc/self/ns/pid").map_err(|_| "Cannot inspect desktop PID namespace")?;
    if origin.get("runId").and_then(Value::as_str) != Some(run_id)
        || origin.get("mount").and_then(Value::as_str) == mount.to_str()
        || origin.get("pid").and_then(Value::as_str) == pid.to_str()
        || origin.get("mount").and_then(Value::as_str).is_none()
        || origin.get("pid").and_then(Value::as_str).is_none()
        || std::env::var("DISPLAY").as_deref() != Ok(":91")
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("XAUTHORITY").as_deref() != Some(run.join("r/Xauthority").as_os_str())
        || std::env::var("DBUS_SESSION_BUS_ADDRESS").as_deref()
            != Ok(format!("unix:path=/tmp/browsey-native-{run_id}/bus").as_str())
    {
        return Err("Refuse desktop services outside the exact isolated display/bus/namespaces");
    }
    let home = origin
        .get("home")
        .and_then(Value::as_str)
        .ok_or("Missing desktop origin home")?;
    match fs::symlink_metadata(Path::new(home).join(".config")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err("Personal desktop configuration is visible"),
    }
    let mut bytes = Vec::new();
    workspaces::private_file(&run.join("artifacts/isolation.json"))?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read desktop isolation proof")?;
    let proof: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid desktop isolation proof")?;
    if proof.get("runId").and_then(Value::as_str) != Some(run_id)
        || [
            "cookieRequired",
            "privateBus",
            "originNamespacesDifferent",
            "personalConfigAbsent",
        ]
        .iter()
        .any(|key| proof.get(key).and_then(Value::as_bool) != Some(true))
    {
        return Err("Desktop isolation was not independently demonstrated");
    }
    Ok(())
}

// Read this exact private catalog only; never adopt another mount's trash IDs.
fn catalog(roots: &[String], profile: &Path) -> Result<BTreeMap<String, String>, &'static str> {
    let info = profile.join("data/Trash/info");
    no_links(&info)?;
    let entries = match fs::read_dir(&info) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(_) => return Err("Cannot inspect owned trash catalog"),
    };
    let mut result = BTreeMap::new();
    for entry in entries {
        if result.len() >= 16 {
            return Err("Owned trash catalog exceeds its entry bound");
        }
        let entry = entry.map_err(|_| "Cannot inspect owned trash entry")?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid private trash ID")?;
        if !name.ends_with(".trashinfo") {
            return Err("Unexpected private trash metadata");
        }
        let mut text = String::new();
        workspaces::private_file(&path)?
            .take(4097)
            .read_to_string(&mut text)
            .map_err(|_| "Cannot read owned trash metadata")?;
        if text.len() > 4096 || !text.starts_with("[Trash Info]\n") {
            return Err("Invalid bounded trash metadata");
        }
        let encoded = text
            .lines()
            .find_map(|line| line.strip_prefix("Path="))
            .ok_or("Missing original trash path")?;
        let decoded = url::Url::parse(&format!("file://{encoded}"))
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .ok_or("Invalid encoded trash original")?;
        let original = decoded
            .to_str()
            .ok_or("Unsupported original trash encoding")?;
        check_path(roots, original)?;
        no_links(&decoded)?;
        let stored = profile.join("data/Trash/files").join(
            name.strip_suffix(".trashinfo")
                .ok_or("Invalid trash suffix")?,
        );
        no_links(&stored)?;
        result.insert(
            path.to_string_lossy().into_owned(),
            stored.to_string_lossy().into_owned(),
        );
    }
    Ok(result)
}

pub(super) fn authorize(
    roots: &[String],
    profile: &Path,
    mode: Option<&str>,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    let special = matches!(
        command,
        "copy_paths_to_system_clipboard"
            | "clear_system_clipboard"
            | "system_clipboard_paths"
            | "move_to_trash"
            | "move_to_trash_many"
            | "list_trash"
            | "restore_trash_items"
            | "purge_trash_items"
            | "empty_trash"
    ) || command == "list_facets"
        && body.get("scope").and_then(Value::as_str) == Some("trash");
    let mut paths = Vec::new();
    if collect_paths(body, "", &mut paths).is_err() {
        return special.then_some(Err("Invalid desktop arguments"));
    }
    let trash_paths = paths
        .iter()
        .any(|path| path.starts_with(&format!("{}/data/Trash/", profile.display())));
    if !special && !trash_paths {
        return None;
    }
    Some((|| {
        if mode != Some("desktop-services") {
            return Err("Desktop services require separately approved isolation");
        }
        if matches!(
            command,
            "copy_paths_to_system_clipboard" | "move_to_trash" | "move_to_trash_many"
        ) {
            if paths.is_empty() {
                return Err("Explicit owned desktop sources required");
            }
            for path in paths {
                check_path(roots, path)?;
                if path.starts_with("rclone://") {
                    return Err("Desktop services are local-only");
                }
                no_links(Path::new(path))?;
            }
            return Ok(());
        }
        if matches!(command, "clear_system_clipboard" | "system_clipboard_paths") {
            if body.as_object().is_none_or(|o| !o.is_empty()) {
                return Err("Clipboard inspection/clear takes no arguments");
            }
            return Ok(());
        }
        // Reject forged/outside identifiers before opening even the private catalog.
        if matches!(command, "restore_trash_items" | "purge_trash_items") {
            let ids = body
                .get("ids")
                .and_then(Value::as_array)
                .ok_or("Explicit private trash IDs required")?;
            if ids.is_empty() || ids.len() > 16 {
                return Err("Bounded private trash IDs required");
            }
            for id in ids {
                let id = id.as_str().ok_or("Invalid private trash ID")?;
                super::segments(id)?;
                if Path::new(id).parent() != Some(profile.join("data/Trash/info").as_path()) {
                    return Err("Outside private trash ID");
                }
            }
            let known = catalog(roots, profile)?;
            for id in ids {
                if !known.contains_key(id.as_str().ok_or("Invalid trash ID")?) {
                    return Err("Unregistered private trash ID");
                }
            }
            return Ok(());
        }
        let known = catalog(roots, profile)?;
        if special {
            return Ok(());
        }
        if !matches!(
            command,
            "context_menu_actions"
                | "entry_times_cmd"
                | "entry_kind_cmd"
                | "entry_extra_metadata_cmd"
                | "get_permissions"
                | "get_permissions_batch"
                | "dir_sizes"
                | "get_thumbnail"
        ) {
            return Err("Only metadata inspection is allowed on private trash paths");
        }
        for path in paths {
            if !known.values().any(|value| value == path) {
                return Err("Unknown private trash payload");
            }
            no_links(Path::new(path))?;
        }
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn ordinary_sessions_and_forged_ids_fail_before_catalog_io() {
        let roots =
            vec!["/unused/ai_agent_testfolder/.bnt-00000000000040008000000000000000/files".into()];
        let profile = Path::new("/unavailable/profile");
        for command in [
            "copy_paths_to_system_clipboard",
            "system_clipboard_paths",
            "list_trash",
            "empty_trash",
            "move_to_trash_many",
        ] {
            assert!(authorize(&roots, profile, None, command, &json!({}))
                .unwrap()
                .is_err());
        }
        for id in [
            "/personal/Trash/info/file.trashinfo",
            "/unavailable/profile/data/Trash/info/../files/file",
            "/unavailable/profile/data/Trash/info-sibling/file.trashinfo",
        ] {
            assert!(authorize(
                &roots,
                profile,
                Some("desktop-services"),
                "purge_trash_items",
                &json!({"ids":[id]})
            )
            .unwrap()
            .is_err());
        }
        assert!(authorize(
            &roots,
            profile,
            Some("desktop-services"),
            "move_to_trash_many",
            &json!({"paths":["/personal/file"]})
        )
        .unwrap()
        .is_err());
    }
}

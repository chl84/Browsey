//! Fail-closed IPC boundary for opt-in native candidates, not a general sandbox.
//! Production builds contain neither the test overrides nor the environment hook.
use serde_json::Value;
mod desktop;
pub(crate) mod links;
pub(crate) mod probes;
mod workspaces;

const FOLDER: &str = "ai_agent_testfolder";

fn segments(raw: &str) -> Result<Vec<&str>, &'static str> {
    if raw.is_empty() || raw.contains(['\0', '\\']) || raw.ends_with('/') {
        return Err("Invalid native-test path");
    }
    let tail = if let Some(tail) = raw.strip_prefix("rclone://") {
        if !tail.split('/').next().is_some_and(|remote| {
            !remote.is_empty()
                && remote
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b' '))
        }) {
            return Err("Invalid native-test remote");
        }
        tail
    } else {
        raw.strip_prefix('/')
            .ok_or("Native-test paths must be absolute")?
    };
    let parts: Vec<_> = tail.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || matches!(*part, "." | ".."))
    {
        return Err("Native-test paths cannot contain relative/empty components");
    }
    Ok(parts)
}

fn owns(root: &str, path: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn validate_root(root: &str, run_id: &str) -> Result<(), &'static str> {
    let parts = segments(root)?;
    let suffix = [
        FOLDER,
        &format!(".bnt-{}", run_id.replace('-', "")),
        "files",
    ];
    if !parts.ends_with(&suffix) {
        return Err("Only owned files under an existing ai_agent_testfolder are allowed");
    }
    Ok(())
}

fn check_path(roots: &[String], raw: &str) -> Result<(), &'static str> {
    segments(raw)?;
    if roots.iter().any(|root| owns(root, raw)) {
        Ok(())
    } else {
        Err("Path is outside this native-test session")
    }
}

fn collect_paths<'a>(
    value: &'a Value,
    key: &str,
    paths: &mut Vec<&'a str>,
) -> Result<(), &'static str> {
    let path_key = matches!(
        key,
        "path"
            | "paths"
            | "src"
            | "dst"
            | "dest"
            | "destDir"
            | "dest_dir"
            | "source"
            | "sources"
            | "target"
            | "parentPath"
            | "parent_path"
            | "selectionPaths"
            | "selection_paths"
    );
    match value {
        Value::String(raw)
            if matches!(key, "name" | "newName" | "new_name")
                && (raw.is_empty()
                    || raw.contains(['/', '\\', '\0'])
                    || matches!(raw.as_str(), "." | "..")) =>
        {
            return Err("Native-test names must be single safe components");
        }
        Value::String(raw) if path_key || raw.starts_with('/') || raw.contains("://") => {
            paths.push(raw)
        }
        Value::Array(items) => {
            for item in items {
                collect_paths(item, key, paths)?;
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                collect_paths(value, key, paths)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn authorize(roots: &[String], command: &str, body: &Value) -> Result<Vec<String>, &'static str> {
    let private_settings = matches!(
        command,
        "about_info"
            | "get_window_control_policy"
            | "load_shortcuts"
            | "cancel_task"
            | "load_saved_column_widths"
            | "store_column_widths"
            | "undo_action"
            | "redo_action"
            | "inspect_undo_storage"
            | "load_ffmpeg_path"
            | "load_rclone_path"
    ) || ((command.starts_with("load_") || command.starts_with("store_"))
        && matches!(
            command
                .trim_start_matches("load_")
                .trim_start_matches("store_"),
            "show_hidden"
                | "hidden_files_last"
                | "high_contrast"
                | "default_view"
                | "folders_first"
                | "confirm_delete"
                | "sort_field"
                | "sort_direction"
                | "density"
                | "theme_mode"
                | "archive_name"
                | "archive_level"
                | "open_dest_after_extract"
                | "thumb_cache_mb"
                | "mounts_poll_ms"
                | "double_click_ms"
                | "log_level"
                | "video_thumbs"
                | "cloud_thumbs"
                | "hardware_acceleration"
                | "scrollbar_width"
        ));
    let file_command = matches!(
        command,
        "list_dir"
            | "search_stream"
            | "list_facets"
            | "watch_dir"
            | "dir_sizes"
            | "context_menu_actions"
            | "classify_network_uri"
            | "entry_times_cmd"
            | "entry_kind_cmd"
            | "entry_extra_metadata_cmd"
            | "get_permissions"
            | "get_permissions_batch"
            | "get_thumbnail"
            | "create_file"
            | "create_folder"
            | "rename_entry"
            | "rename_entries"
            | "delete_entry"
            | "delete_entries"
            | "network_delete_entries"
            | "network_delete_paths"
            | "set_clipboard_cmd"
            | "resolve_drop_clipboard_mode"
            | "paste_clipboard_cmd"
            | "paste_clipboard_preview"
            | "list_cloud_entries"
            | "stat_cloud_entry"
            | "create_cloud_folder"
            | "create_cloud_file"
            | "delete_cloud_file"
            | "delete_cloud_dir_empty"
            | "delete_cloud_dir_recursive"
            | "rename_cloud_entry"
            | "rename_cloud_entries"
            | "copy_cloud_entry"
            | "move_cloud_entry"
            | "preview_cloud_conflicts"
            | "preview_mixed_transfer_conflicts"
            | "copy_mixed_entries"
            | "move_mixed_entries"
            | "copy_mixed_entry_to"
            | "move_mixed_entry_to"
    );
    if !private_settings && !file_command {
        return Err("Command is not permitted in the native foundation suite");
    }
    if matches!(command, "search_stream" | "watch_dir") {
        let path = body
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Native-test search/watch requires an explicit directory")?;
        check_path(roots, path)?;
    }
    if matches!(command, "paste_clipboard_cmd" | "paste_clipboard_preview")
        && !body.get("input").is_some_and(Value::is_object)
    {
        return Err("Native-test paste requires explicit owned sources");
    }
    if command == "network_delete_entries"
        && body.get("trash").and_then(Value::as_bool) != Some(false)
    {
        return Err("Native-test network deletion must explicitly avoid global trash");
    }
    if command == "list_facets" && body.get("scope").and_then(Value::as_str) != Some("dir") {
        return Err("Native-test facets are directory-only");
    }
    let mut paths = Vec::new();
    collect_paths(body, "", &mut paths)?;
    let empty_context = command == "context_menu_actions"
        && body.get("count").and_then(Value::as_u64) == Some(0)
        && body
            .get("selectionPaths")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty);
    if file_command && paths.is_empty() && !empty_context {
        return Err("Native-test file commands require explicit owned paths");
    }
    for raw in &paths {
        check_path(roots, raw)?;
    }
    Ok(paths.into_iter().map(str::to_string).collect())
}

// Complete lexical/command authorization before any metadata or operation I/O.
#[cfg(any(feature = "native-test", test))]
fn authorize_io(
    roots: &[String],
    command: &str,
    body: &Value,
    mut inspect: impl FnMut(&str) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let mut paths = authorize(roots, command, body)?;
    if matches!(command, "undo_action" | "redo_action") {
        paths.extend(roots.iter().cloned());
    }
    for raw in paths {
        if !raw.starts_with("rclone://") {
            inspect(&raw)?;
        }
    }
    Ok(())
}

#[cfg(any(feature = "native-test", test))]
use std::{
    fs,
    path::{Component, Path},
};

// Check components without following a symlink, including absent creation targets.
#[cfg(any(feature = "native-test", test))]
fn no_links(path: &Path) -> Result<(), &'static str> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err("Invalid native-test path component");
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("Native-test symlinks are forbidden")
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err("Cannot verify native-test path"),
        }
    }
    Ok(())
}

// Recursive operations must not encounter links hidden under an approved directory.
#[cfg(test)]
fn no_tree_links(path: &Path, remaining: &mut usize) -> Result<(), &'static str> {
    no_tree_links_at_depth(path, remaining, 0)
}

#[cfg(test)]
fn no_tree_links_at_depth(
    path: &Path,
    remaining: &mut usize,
    depth: usize,
) -> Result<(), &'static str> {
    if depth > 32 {
        return Err("Native-test tree depth budget exceeded");
    }
    if *remaining == 0 {
        return Err("Native-test tree exceeds the foundation limit");
    }
    *remaining -= 1;
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("Cannot inspect owned native-test tree"),
    };
    if meta.file_type().is_symlink() {
        return Err("Native-test symlinks are forbidden");
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| "Cannot inspect owned native-test directory")? {
            no_tree_links_at_depth(
                &entry
                    .map_err(|_| "Cannot inspect owned native-test entry")?
                    .path(),
                remaining,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FaultPlan {
    id: String,
    operation: String,
    source: Option<String>,
    refresh_target: Option<String>,
    arm_source: Option<String>,
}
struct OwnedFault {
    plan: FaultPlan,
    armed: std::sync::atomic::AtomicBool,
    used: std::sync::atomic::AtomicUsize,
}
impl OwnedFault {
    fn new(plan: FaultPlan) -> Self {
        Self {
            plan,
            armed: std::sync::atomic::AtomicBool::new(false),
            used: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    fn consume(&self) -> bool {
        self.used
            .compare_exchange(
                0,
                1,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
    }
}
fn validate_faults(
    roots: &[String],
    plans: Vec<FaultPlan>,
) -> Result<Vec<OwnedFault>, &'static str> {
    if plans.len() > 8 {
        return Err("Native-test fault plan is too large");
    }
    let mut ids = std::collections::HashSet::new();
    for plan in &plans {
        if plan.id.is_empty()
            || plan.id.len() > 64
            || !plan
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !ids.insert(plan.id.as_str())
            || !matches!(plan.operation.as_str(), "copy" | "move")
        {
            return Err("Invalid native-test fault identity/operation");
        }
        match (&plan.source, &plan.refresh_target, &plan.arm_source) {
            (Some(source), None, None) if source.starts_with("rclone://") => {}
            (None, Some(target), Some(source))
                if !target.starts_with("rclone://") && !source.starts_with("rclone://") => {}
            _ => return Err("Invalid native-test fault shape"),
        }
        for raw in [&plan.source, &plan.refresh_target, &plan.arm_source]
            .into_iter()
            .flatten()
        {
            check_path(roots, raw)?;
            if roots.contains(raw) {
                return Err("Native-test faults need exact generated children");
            }
            if !raw.starts_with("rclone://") {
                no_links(std::path::Path::new(raw))?;
            }
        }
    }
    Ok(plans.into_iter().map(OwnedFault::new).collect())
}
fn apply_faults(faults: &[OwnedFault], command: &str, body: &Value) -> Option<&'static str> {
    use std::sync::atomic::Ordering;
    for fault in faults {
        let copy = fault.plan.operation == "copy";
        let transfer = if copy {
            command == "copy_cloud_entry" || command == "copy_mixed_entry_to"
        } else {
            command == "move_cloud_entry" || command == "move_mixed_entry_to"
        };
        let source = body.get("src").and_then(Value::as_str);
        if transfer && fault.plan.source.as_deref() == source && source.is_some() && fault.consume()
        {
            return Some("Injected owned source failure");
        }
        let batch = command == "paste_clipboard_cmd"
            && body
                .get("input")
                .and_then(|v| v.get("mode"))
                .and_then(Value::as_str)
                == Some(if copy { "copy" } else { "cut" });
        let batch_match = batch
            && body
                .get("input")
                .and_then(|v| v.get("paths"))
                .and_then(Value::as_array)
                .is_some_and(|paths| {
                    paths
                        .iter()
                        .any(|p| p.as_str() == fault.plan.arm_source.as_deref())
                });
        if fault.plan.arm_source.is_some()
            && ((transfer && source == fault.plan.arm_source.as_deref()) || batch_match)
        {
            fault.armed.store(true, Ordering::SeqCst);
        }
        if command == "list_dir"
            && body.get("path").and_then(Value::as_str) == fault.plan.refresh_target.as_deref()
            && fault.plan.refresh_target.is_some()
            && fault.armed.load(Ordering::SeqCst)
            && fault.consume()
        {
            return Some("Injected owned refresh failure");
        }
    }
    None
}

#[cfg(feature = "native-test")]
mod enabled {
    use super::*;
    use once_cell::sync::OnceCell;
    use serde::Deserialize;
    use std::env;
    use tauri::ipc::{Invoke, InvokeBody};
    use tauri::Manager;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Session {
        run_id: String,
        data_roots: Vec<String>,
        profile: String,
        #[serde(default)]
        faults: Vec<FaultPlan>,
        #[serde(default)]
        probes: Vec<probes::Plan>,
        #[serde(default)]
        links: links::Plan,
        #[serde(default)]
        workspace_source: Option<String>,
        #[serde(default)]
        desktop: Option<String>,
    }
    static SESSION: OnceCell<Session> = OnceCell::new();
    static FAULTS: OnceCell<Vec<OwnedFault>> = OnceCell::new();

    pub(crate) fn initialize() -> Result<(), &'static str> {
        let raw =
            env::var("BROWSEY_NATIVE_TEST_SESSION").map_err(|_| "Missing native-test session")?;
        if raw.len() > 32_768 {
            return Err("Native-test session is too large");
        }
        let mut session: Session =
            serde_json::from_str(&raw).map_err(|_| "Invalid native-test session")?;
        if session.run_id.len() != 36
            || !session
                .run_id
                .bytes()
                .all(|c| c.is_ascii_hexdigit() || c == b'-')
            || session.data_roots.is_empty()
            || session.data_roots.len() > 5
            || session.data_roots[0].starts_with("rclone://")
        {
            return Err("Invalid native-test identity/local root");
        }
        for root in &session.data_roots {
            validate_root(root, &session.run_id)?;
            if !root.starts_with("rclone://") {
                no_links(Path::new(root))?;
                if !Path::new(root).is_dir() {
                    return Err("Native-test data root must already exist");
                }
            }
        }
        links::validate(&session.data_roots, &session.links)?;
        workspaces::validate(&session.data_roots, session.workspace_source.as_deref())?;
        let faults = validate_faults(&session.data_roots, std::mem::take(&mut session.faults))?;
        probes::initialize(&session.data_roots, std::mem::take(&mut session.probes))?;
        let local_run = Path::new(&session.data_roots[0])
            .parent()
            .ok_or("Missing local run")?;
        if Path::new(&session.profile) != local_run.join("profile") {
            return Err("Profile is outside the owned local run");
        }
        desktop::validate_isolation(session.desktop.as_deref(), &session.run_id, local_run)?;
        for (key, suffix) in [
            ("XDG_DATA_HOME", "data"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_STATE_HOME", "state"),
            ("HOME", "home"),
        ] {
            let expected = Path::new(&session.profile).join(suffix);
            if env::var_os(key).as_deref() != Some(expected.as_os_str()) {
                return Err("Native-test XDG profile mismatch");
            }
            no_links(&expected)?;
            if !expected.is_dir() {
                return Err("Private native-test profile directory is missing");
            }
        }
        for (key, suffix) in [("XDG_RUNTIME_DIR", "r"), ("TMPDIR", "t")] {
            let expected = local_run.join(suffix);
            if env::var_os(key).as_deref() != Some(expected.as_os_str()) {
                return Err("Native-test runtime/temp directory mismatch");
            }
            no_links(&expected)?;
            if !expected.is_dir() {
                return Err("Private runtime/temp directory is missing");
            }
        }
        let undo = Path::new(&session.profile).join("data/browsey/undo-sessions");
        if env::var_os("BROWSEY_UNDO_DIR").as_deref() != Some(undo.as_os_str()) {
            return Err("Native-test undo storage is outside the profile");
        }
        let config =
            env::var("RCLONE_CONFIG").map_err(|_| "Native-test rclone config must be explicit")?;
        if Path::new(&config) != Path::new(&session.profile).join("config/rclone.conf") {
            return Err("Native-test rclone config is outside the profile");
        }
        no_links(Path::new(&config))?;
        let conn =
            crate::db::open().map_err(|_| "Cannot initialize the private native-test database")?;
        crate::db::set_setting_bool(
            &conn,
            "cloudEnabled",
            session
                .data_roots
                .iter()
                .any(|p| p.starts_with("rclone://")),
        )
        .map_err(|_| "Cannot configure private cloud setting")?;
        crate::db::set_setting_string(&conn, "defaultView", "list")
            .map_err(|_| "Cannot configure private view setting")?;
        FAULTS
            .set(faults)
            .map_err(|_| "Native-test faults already initialized")?;
        SESSION
            .set(session)
            .map_err(|_| "Native-test session already initialized")
    }

    pub(crate) fn working_copies_enabled() -> bool {
        SESSION
            .get()
            .is_some_and(|session| session.workspace_source.is_some())
    }

    pub(crate) fn intercept(invoke: Invoke) -> Option<Invoke> {
        let session = SESSION
            .get()
            .expect("native-test session was initialized before the webview");
        let command = invoke.message.command();
        let override_value = match command {
            "native_test_status" => Some(serde_json::json!({"runId":session.run_id,
                "pid":std::process::id(), "scope":"owned-files-only", "watcher":session.desktop.as_deref()==Some("watchers"),
                "watcherActive":invoke.message.webview_ref().state::<crate::watcher::WatchState>().native_active(), "desktopMode":session.desktop,
                "probes":probes::status(),
                "cancelTasks":invoke.message.webview_ref().state::<crate::tasks::CancelState>().native_active_count(),
                "faults":FAULTS.get().expect("faults initialized").iter().map(|fault| serde_json::json!({
                    "id":fault.plan.id, "uses":fault.used.load(std::sync::atomic::Ordering::SeqCst),
                    "armed":fault.armed.load(std::sync::atomic::Ordering::SeqCst)})).collect::<Vec<_>>()})),
            "get_startup_path" | "load_start_dir" => Some(serde_json::json!(session.data_roots[0])),
            // Only disposable roots, never the user's saved bookmarks.
            "get_bookmarks" => Some(Value::Array(session.data_roots.iter().enumerate()
                .map(|(index, path)| serde_json::json!({"label":format!("Fixture {}",index+1), "path":path})).collect())),
            "list_mounts"
            | "list_cloud_remotes"
            | "list_network_entries"
            | "list_saved_network_connections" => Some(serde_json::json!([])),
            "load_cloud_enabled" => Some(serde_json::json!(session
                .data_roots
                .iter()
                .any(|p| p.starts_with("rclone://")))),
            "load_system_theme" => Some(Value::Null),
            "copy_paths_to_system_clipboard" | "clear_system_clipboard"
                if session.desktop.as_deref() != Some("desktop-services") => Some(Value::Null),
            // The normal watcher has home-directory fallback/discovery. Keep it off;
            // the suite explicitly refreshes after fixture changes and navigation.
            "watch_dir" if session.desktop.as_deref() != Some("watchers") => Some(Value::Null),
            "system_clipboard_paths" if session.desktop.as_deref() != Some("desktop-services") => Some(serde_json::json!({"mode":"copy", "paths":[]})),
            _ => None,
        };
        if let Some(value) = override_value {
            invoke.resolver.resolve(value);
            return None;
        }
        let result = match invoke.message.payload() {
            InvokeBody::Json(body) => desktop::authorize(
                &session.data_roots,
                Path::new(&session.profile),
                session.desktop.as_deref(),
                command,
                body,
            )
            .unwrap_or_else(|| {
                workspaces::authorize(
                    &session.data_roots,
                    Path::new(&session.profile),
                    session.workspace_source.as_deref(),
                    command,
                    body,
                )
                .unwrap_or_else(|| {
                    authorize_io(&session.data_roots, command, body, |raw| {
                        links::inspect(&session.links, Path::new(raw))
                    })
                })
            }),
            _ => Err("Native-test IPC must use JSON"),
        };
        if let Err(reason) = result {
            // No arguments, usernames, addresses, file contents or credentials in audit logs.
            tracing::warn!(command, "native-test scope rejected a command");
            invoke
                .resolver
                .reject(serde_json::json!({"code":"native_test_scope_denied", "message":reason}));
            None
        } else {
            // Scope/path authorization above always precedes any one-use fault.
            if let InvokeBody::Json(body) = invoke.message.payload() {
                if let Some(reason) =
                    apply_faults(FAULTS.get().expect("faults initialized"), command, body)
                {
                    invoke
                        .resolver
                        .reject(serde_json::json!({"code":"io_error", "message":reason}));
                    return None;
                }
            }
            Some(invoke)
        }
    }

    pub(crate) fn check_desktop_clipboard_paths(paths: &[String]) -> Result<(), &'static str> {
        let session = SESSION.get().ok_or("Missing owned clipboard session")?;
        if session.desktop.as_deref() != Some("desktop-services") {
            return Err("Native system clipboard is disabled");
        }
        for path in paths {
            check_path(&session.data_roots, path)?;
            if path.starts_with("rclone://") {
                return Err("System clipboard requires owned local paths");
            }
            no_links(Path::new(path))?;
        }
        Ok(())
    }

    pub(crate) fn watchers_enabled() -> bool {
        SESSION
            .get()
            .is_some_and(|session| session.desktop.as_deref() == Some("watchers"))
    }

    pub(crate) fn owned_watch_path(path: Option<&str>) -> Result<std::path::PathBuf, &'static str> {
        let session = SESSION.get().ok_or("Missing owned watcher session")?;
        if !watchers_enabled() {
            return Err("Owned watcher mode is disabled");
        }
        super::validate_owned_watch_path(&session.data_roots, path)
    }
}

fn validate_owned_watch_path(
    roots: &[String],
    path: Option<&str>,
) -> Result<std::path::PathBuf, &'static str> {
    let raw = path.ok_or("Explicit owned watch directory required")?;
    check_path(roots, raw)?;
    if raw.starts_with("rclone://") {
        return Err("Owned watcher mode is local-only");
    }
    let target = Path::new(raw);
    no_links(target)?;
    if !std::fs::metadata(target)
        .map_err(|_| "Owned watch directory unavailable")?
        .is_dir()
    {
        return Err("Owned watch target must be a directory");
    }
    Ok(target.to_path_buf())
}

#[cfg(feature = "native-test")]
pub(crate) use enabled::check_desktop_clipboard_paths;
#[cfg(feature = "native-test")]
pub(crate) use enabled::{initialize, intercept, working_copies_enabled};
#[cfg(feature = "native-test")]
pub(crate) use enabled::{owned_watch_path, watchers_enabled};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const ID: &str = "00000000-0000-4000-8000-000000000000";
    fn roots() -> Vec<String> {
        vec![
            format!("/approved/{FOLDER}/.bnt-{}/files", ID.replace('-', "")),
            format!("rclone://Test/{FOLDER}/.bnt-{}/files", ID.replace('-', "")),
        ]
    }

    #[test]
    fn watcher_scope_rejects_missing_cloud_outside_and_traversal_before_io() {
        let roots = roots();
        assert_eq!(
            validate_owned_watch_path(&roots, None).unwrap_err(),
            "Explicit owned watch directory required"
        );
        for path in [
            "/personal/home",
            "/approved/ai_agent_testfolder-sibling",
            "relative",
            "/approved/../home",
        ] {
            assert!(validate_owned_watch_path(&roots, Some(path)).is_err());
        }
        assert_eq!(
            validate_owned_watch_path(&roots, Some(&roots[1])).unwrap_err(),
            "Owned watcher mode is local-only"
        );
        for body in [
            json!({}),
            json!({"path":null}),
            json!({"path":"/personal/home"}),
        ] {
            assert!(authorize(&roots, "watch_dir", &body).is_err());
        }
    }

    #[test]
    fn native_scope_accepts_only_exact_folder_owned_run_and_component_boundaries() {
        let roots = roots();
        for root in &roots {
            validate_root(root, ID).unwrap();
            check_path(&roots, &format!("{root}/nested/file.txt")).unwrap();
        }
        for bad in [
            "/personal/file",
            "/approved/ai_agent_test_folder",
            "rclone://Test/personal/file",
        ] {
            assert!(check_path(&roots, bad).is_err());
        }
        assert!(check_path(&roots, &format!("{}-sibling/file", roots[0])).is_err());
        assert!(validate_root(&roots[0].replace(FOLDER, "ai_agent_test_folder"), ID).is_err());
        assert!(validate_root(&roots[0].replace(&ID.replace('-', ""), "other"), ID).is_err());
    }

    #[test]
    fn native_scope_rejects_traversal_uri_aliases_and_implicit_home() {
        let roots = roots();
        for suffix in [
            "/../personal",
            "/./file",
            "//file",
            "/file\\other",
            "/file\0",
        ] {
            assert!(check_path(&roots, &format!("{}{suffix}", roots[0])).is_err());
        }
        for path in [
            "~",
            "file:///approved",
            "sftp://user@host/folder",
            "relative",
        ] {
            assert!(check_path(&roots, path).is_err());
        }
        assert!(authorize(&roots, "list_dir", &json!({})).is_err());
        assert!(authorize(
            &roots,
            "create_file",
            &json!({"path":roots[0], "name":"../../personal"})
        )
        .is_err());
        assert!(authorize(
            &roots,
            "list_facets",
            &json!({"scope":"trash","path":roots[0]})
        )
        .is_err());
    }

    #[test]
    fn native_scope_validates_all_mixed_sources_and_destinations() {
        let roots = roots();
        authorize(
            &roots,
            "copy_mixed_entries",
            &json!({"sources":[roots[0]],"destDir":roots[1]}),
        )
        .unwrap();
        assert!(authorize(
            &roots,
            "copy_mixed_entries",
            &json!({"sources":[roots[0],"/personal"],"destDir":roots[1]})
        )
        .is_err());
        assert!(authorize(&roots, "paste_clipboard_cmd", &json!({"dest":roots[0]})).is_err());
        authorize(
            &roots,
            "paste_clipboard_cmd",
            &json!({"dest":roots[0],"input":{"paths":[roots[0]],"mode":"copy"}}),
        )
        .unwrap();
    }

    #[test]
    fn native_scope_denies_unknown_commands_global_trash_devices_and_external_programs() {
        let roots = roots();
        for command in [
            "unknown_new_command",
            "format_removable_partition",
            "mount_usb_volume",
            "open_entry",
            "connect_network_uri",
            "empty_trash",
            "move_to_trash",
            "trash_cloud_entries",
            "store_rclone_path",
        ] {
            assert!(authorize(&roots, command, &json!({"path":roots[0]})).is_err());
        }
        authorize(&roots, "load_shortcuts", &json!({})).unwrap();
        authorize(&roots, "undo_action", &json!({})).unwrap();
    }

    #[test]
    fn native_scope_denies_entire_request_before_any_io() {
        let roots = roots();
        let mut inspected = Vec::new();
        for (command, body) in [
            ("search_stream", json!({})),
            ("search_stream", json!({"path":null})),
            ("search_stream", json!({"path":"/synthetic/outside"})),
            (
                "search_stream",
                json!({"path":format!("{}/../file", roots[0])}),
            ),
            (
                "search_stream",
                json!({"path":"rclone://Generated/personal"}),
            ),
            (
                "copy_mixed_entries",
                json!({"sources":[roots[0], "/synthetic/outside"], "destDir":roots[0]}),
            ),
            (
                "rename_entries",
                json!({"entries":[{"path":roots[0], "newName":"../escape"}]}),
            ),
            (
                "context_menu_actions",
                json!({"count":1, "selectionPaths":["relative"]}),
            ),
            (
                "list_dir",
                json!({"path":format!("{}-sibling/file", roots[0])}),
            ),
            ("list_dir", json!({"path":format!("{}/../file", roots[0])})),
            ("list_dir", json!({"path":"file:///synthetic/alias"})),
            ("open_entry", json!({"path":roots[0]})),
            (
                "network_delete_entries",
                json!({"paths":[roots[0]], "trash":true}),
            ),
            ("network_delete_entries", json!({"paths":[roots[0]]})),
        ] {
            assert!(
                authorize_io(&roots, command, &body, |raw| {
                    inspected.push(raw.to_string());
                    Ok(())
                })
                .is_err(),
                "{command} must be denied"
            );
        }
        assert!(
            inspected.is_empty(),
            "Denied requests must not reach metadata I/O"
        );
        authorize_io(
            &roots,
            "search_stream",
            &json!({"path":roots[0], "query":"alpha"}),
            |raw| {
                inspected.push(raw.to_string());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(inspected, vec![roots[0].clone()]);
        authorize_io(
            &roots,
            "search_stream",
            &json!({"path":roots[1], "query":"alpha"}),
            |_| panic!("Cloud search must not reach local metadata I/O"),
        )
        .unwrap();
    }

    #[test]
    fn native_scope_tree_depth_is_bounded_before_recursive_dispatch() {
        let temp = std::env::temp_dir().join(format!(
            "browsey-native-depth-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = temp.join("owned");
        let deep = (0..33).fold(root.clone(), |p, _| p.join("d"));
        fs::create_dir_all(&deep).unwrap();
        let rejected = no_tree_links(&root, &mut 4096).is_err();
        no_tree_links(&deep, &mut 4096).unwrap();
        fs::remove_dir_all(temp).unwrap();
        assert!(rejected);
    }

    #[cfg(unix)]
    #[test]
    fn native_scope_rechecks_replaced_and_nested_symlinks_before_dispatch() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};
        let temp = std::env::temp_dir().join(format!(
            "browsey-native-scope-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let files = temp
            .join(FOLDER)
            .join(format!(".bnt-{}", ID.replace('-', "")))
            .join("files");
        fs::create_dir_all(files.join("source/nested")).unwrap();
        fs::create_dir(files.join("referent")).unwrap();
        fs::write(files.join("referent/keep.txt"), b"generated sentinel").unwrap();
        let roots = vec![files.to_str().unwrap().to_string()];
        let body = json!({"sources":[files.join("source").to_str().unwrap()], "destDir":roots[0]});
        let check = |raw: &str| {
            no_links(Path::new(raw))?;
            no_tree_links(Path::new(raw), &mut 4096)
        };
        authorize_io(&roots, "copy_mixed_entries", &body, check).unwrap();
        // Replacement between requests; no claim of atomic defense during I/O.
        fs::remove_dir(files.join("source/nested")).unwrap();
        symlink(files.join("referent"), files.join("source/nested")).unwrap();
        assert!(authorize_io(&roots, "copy_mixed_entries", &body, check).is_err());
        assert!(no_links(&files.join("source/nested/keep.txt")).is_err());
        // Broken links are rejected as well, without following a referent.
        symlink(files.join("missing"), files.join("broken")).unwrap();
        assert!(no_tree_links(&files.join("broken"), &mut 4096).is_err());
        assert!(no_tree_links(&files, &mut 0).is_err());
        assert_eq!(
            fs::read(files.join("referent/keep.txt")).unwrap(),
            b"generated sentinel"
        );
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn native_fault_plans_are_bounded_owned_and_deny_unknown_fields() {
        let roots = roots();
        let plan = |id: &str, source: &str| {
            serde_json::from_value::<FaultPlan>(json!({"id":id,"operation":"copy","source":source}))
                .unwrap()
        };
        let owned = format!("{}/failed.txt", roots[1]);
        assert!(validate_faults(&roots, vec![plan("one", &owned)]).is_ok());
        for source in ["rclone://Test/outside/failed.txt", roots[1].as_str()] {
            assert!(validate_faults(&roots, vec![plan("one", source)]).is_err());
        }
        assert!(validate_faults(
            &roots,
            (0..9)
                .map(|i| plan(&format!("fault-{i}"), &owned))
                .collect()
        )
        .is_err());
        assert!(validate_faults(&roots, vec![plan("same", &owned), plan("same", &owned)]).is_err());
        assert!(serde_json::from_value::<FaultPlan>(
            json!({"id":"one","operation":"copy","source":owned,"arbitrary":true})
        )
        .is_err());
        assert!(validate_faults(
            &roots,
            vec![FaultPlan {
                id: "one".into(),
                operation: "delete".into(),
                source: Some(owned),
                refresh_target: None,
                arm_source: None
            }]
        )
        .is_err());
    }
    #[test]
    fn native_source_fault_requires_exact_operation_command_and_single_use() {
        let roots = roots();
        let source = format!("{}/failed.txt", roots[1]);
        let faults = validate_faults(
            &roots,
            vec![serde_json::from_value(
                json!({"id":"source-fault","operation":"copy","source":source}),
            )
            .unwrap()],
        )
        .unwrap();
        for command in [
            "move_cloud_entry",
            "list_cloud_entries",
            "delete_cloud_file",
            "unknown",
        ] {
            assert!(apply_faults(&faults, command, &json!({"src":source})).is_none());
        }
        assert!(apply_faults(
            &faults,
            "copy_cloud_entry",
            &json!({"src":format!("{}/other.txt",roots[1])})
        )
        .is_none());
        assert_eq!(
            apply_faults(&faults, "copy_cloud_entry", &json!({"src":source})),
            Some("Injected owned source failure")
        );
        assert!(apply_faults(&faults, "copy_cloud_entry", &json!({"src":source})).is_none());
        assert_eq!(faults[0].used.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    #[test]
    fn native_refresh_fault_cannot_fire_during_navigation_or_before_exact_transfer() {
        let roots = roots();
        let source = format!("{}/good.txt", roots[0]);
        let target = format!("{}/target", roots[0]);
        let faults=validate_faults(&roots,vec![serde_json::from_value(json!({"id":"refresh-fault","operation":"copy","refreshTarget":target,"armSource":source})).unwrap()]).unwrap();
        let listing = json!({"path":target});
        assert!(apply_faults(&faults, "list_dir", &listing).is_none());
        assert!(apply_faults(
            &faults,
            "paste_clipboard_cmd",
            &json!({"input":{"mode":"cut","paths":[source]}})
        )
        .is_none());
        assert!(apply_faults(&faults, "list_dir", &listing).is_none());
        assert!(apply_faults(
            &faults,
            "paste_clipboard_cmd",
            &json!({"input":{"mode":"copy","paths":[source]}})
        )
        .is_none());
        assert!(apply_faults(&faults, "list_dir", &json!({"path":roots[0]})).is_none());
        assert_eq!(
            apply_faults(&faults, "list_dir", &listing),
            Some("Injected owned refresh failure")
        );
        assert!(apply_faults(&faults, "list_dir", &listing).is_none());
        assert_eq!(faults[0].used.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    #[test]
    fn unauthorized_fault_request_cannot_consume_or_arm_an_owned_plan() {
        let roots = roots();
        let source = format!("{}/failed.txt", roots[1]);
        let faults = validate_faults(
            &roots,
            vec![serde_json::from_value(
                json!({"id":"source-fault","operation":"copy","source":source}),
            )
            .unwrap()],
        )
        .unwrap();
        let body = json!({"src":source,"dst":"/unapproved/target"});
        let authorized = authorize_io(&roots, "copy_cloud_entry", &body, |_| Ok(()));
        assert!(authorized.is_err());
        if authorized.is_ok() {
            apply_faults(&faults, "copy_cloud_entry", &body);
        }
        assert_eq!(faults[0].used.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}

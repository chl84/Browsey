mod accelerator;
mod model;

use rusqlite::Connection;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

use model::{ShortcutCommandDefinition, SHORTCUT_COMMANDS};

const SHORTCUTS_SETTING_KEY: &str = "shortcutBindingsV1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    pub command_id: String,
    pub label: String,
    pub context: String,
    pub default_accelerator: String,
    pub accelerator: String,
}

fn find_definition(command_id: &str) -> Option<&'static ShortcutCommandDefinition> {
    SHORTCUT_COMMANDS.iter().find(|def| def.id == command_id)
}

fn build_bindings(overrides: &HashMap<String, String>) -> Result<Vec<ShortcutBinding>, String> {
    let mut used: HashMap<String, (String, String)> = HashMap::new();
    let mut out = Vec::with_capacity(SHORTCUT_COMMANDS.len());

    for def in SHORTCUT_COMMANDS {
        let default_accelerator = accelerator::canonicalize_accelerator(def.default_accelerator)
            .map_err(|e| format!("invalid default shortcut for {}: {e}", def.id))?;
        let accelerator = if let Some(override_accel) = overrides.get(def.id) {
            accelerator::canonicalize_accelerator(override_accel)
                .map_err(|e| format!("invalid shortcut for {}: {e}", def.id))?
        } else {
            default_accelerator.clone()
        };

        let used_key = format!(
            "{}::{}",
            def.context.to_ascii_lowercase(),
            accelerator.to_ascii_lowercase()
        );
        if let Some((existing_id, existing_label)) = used.get(&used_key) {
            if existing_id != def.id {
                return Err(format!(
                    "Shortcut '{accelerator}' is already used by '{existing_label}'"
                ));
            }
        }
        used.insert(used_key, (def.id.to_string(), def.label.to_string()));

        out.push(ShortcutBinding {
            command_id: def.id.to_string(),
            label: def.label.to_string(),
            context: def.context.to_string(),
            default_accelerator,
            accelerator,
        });
    }

    Ok(out)
}

fn load_overrides(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let raw =
        crate::db::get_setting_string(conn, SHORTCUTS_SETTING_KEY).map_err(|e| e.to_string())?;
    let Some(raw) = raw else {
        return Ok(HashMap::new());
    };

    let parsed: HashMap<String, String> = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse shortcut settings: {e}"))?;

    let mut out = HashMap::new();
    for (command_id, accelerator) in parsed {
        if find_definition(&command_id).is_none() {
            continue;
        }
        let canonical = accelerator::canonicalize_accelerator(&accelerator)
            .map_err(|e| format!("invalid shortcut for '{command_id}': {e}"))?;
        out.insert(command_id, canonical);
    }
    Ok(out)
}

fn save_overrides(conn: &Connection, overrides: &HashMap<String, String>) -> Result<(), String> {
    let mut stable: BTreeMap<&str, &str> = BTreeMap::new();
    for (command_id, accelerator) in overrides {
        stable.insert(command_id.as_str(), accelerator.as_str());
    }
    let payload = serde_json::to_string(&stable)
        .map_err(|e| format!("failed to serialize shortcut settings: {e}"))?;
    crate::db::set_setting_string(conn, SHORTCUTS_SETTING_KEY, &payload).map_err(|e| e.to_string())
}

fn load_overrides_or_default(conn: &Connection) -> HashMap<String, String> {
    match load_overrides(conn) {
        Ok(overrides) => overrides,
        Err(_) => HashMap::new(),
    }
}

pub fn load_shortcuts(conn: &Connection) -> Result<Vec<ShortcutBinding>, String> {
    let overrides = load_overrides_or_default(conn);
    let bindings = build_bindings(&overrides)?;
    Ok(bindings)
}

pub fn set_shortcut_binding(
    conn: &Connection,
    command_id: &str,
    accelerator: &str,
) -> Result<Vec<ShortcutBinding>, String> {
    if find_definition(command_id).is_none() {
        return Err(format!("unknown shortcut command '{command_id}'"));
    }

    let canonical = accelerator::canonicalize_accelerator(accelerator)?;
    let mut overrides = load_overrides_or_default(conn);
    overrides.insert(command_id.to_string(), canonical);

    let bindings = build_bindings(&overrides)?;
    save_overrides(conn, &overrides)?;
    Ok(bindings)
}

pub fn reset_shortcut_binding(
    conn: &Connection,
    command_id: &str,
) -> Result<Vec<ShortcutBinding>, String> {
    if find_definition(command_id).is_none() {
        return Err(format!("unknown shortcut command '{command_id}'"));
    }

    let mut overrides = load_overrides_or_default(conn);
    overrides.remove(command_id);
    let bindings = build_bindings(&overrides)?;
    save_overrides(conn, &overrides)?;
    Ok(bindings)
}

pub fn reset_all_shortcuts(conn: &Connection) -> Result<Vec<ShortcutBinding>, String> {
    let overrides = HashMap::new();
    let bindings = build_bindings(&overrides)?;
    save_overrides(conn, &overrides)?;
    Ok(bindings)
}

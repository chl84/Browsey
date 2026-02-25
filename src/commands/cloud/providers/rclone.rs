use super::super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    rclone_cli::{RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand},
    types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind, CloudRemote},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(in crate::commands::cloud) struct RcloneCloudProvider {
    cli: RcloneCli,
}

#[allow(dead_code)]
impl RcloneCloudProvider {
    pub fn new(cli: RcloneCli) -> Self {
        Self { cli }
    }

    pub fn cli(&self) -> &RcloneCli {
        &self.cli
    }
}

impl CloudProvider for RcloneCloudProvider {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>> {
        let output = self
            .cli
            .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::ListRemotes))
            .map_err(map_rclone_error)?;
        let remote_ids = parse_listremotes_plain(&output.stdout).map_err(|message| {
            CloudCommandError::new(CloudCommandErrorCode::UnknownError, message)
        })?;
        let config_dump = self
            .cli
            .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::ConfigDump))
            .map_err(map_rclone_error)?;
        let type_map = parse_config_dump_types(&config_dump.stdout).map_err(|message| {
            CloudCommandError::new(CloudCommandErrorCode::InvalidConfig, message)
        })?;

        let mut remotes = Vec::new();
        let mut seen = HashSet::new();
        for remote_id in remote_ids {
            if !seen.insert(remote_id.clone()) {
                continue;
            }
            let Some(provider) = type_map
                .get(&remote_id)
                .and_then(|ty| classify_provider_kind(ty))
            else {
                continue;
            };
            remotes.push(CloudRemote {
                id: remote_id.clone(),
                label: format_remote_label(&remote_id, provider),
                provider,
                root_path: format!("rclone://{remote_id}"),
                capabilities: CloudCapabilities::v1_core_rw(),
            });
        }
        remotes.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(remotes)
    }

    fn list_dir(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
        let output = self
            .cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::LsJson).arg(path.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        let items = parse_lsjson_items(&output.stdout).map_err(|message| {
            CloudCommandError::new(CloudCommandErrorCode::UnknownError, message)
        })?;
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let child_path = path.child_path(&item.name).map_err(|error| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidPath,
                    format!("Invalid entry name from rclone lsjson: {error}"),
                )
            })?;
            entries.push(CloudEntry {
                name: item.name,
                path: child_path.to_string(),
                kind: if item.is_dir {
                    CloudEntryKind::Dir
                } else {
                    CloudEntryKind::File
                },
                size: if item.is_dir { None } else { item.size },
                modified: item.mod_time,
                capabilities: CloudCapabilities::v1_core_rw(),
            });
        }
        entries.sort_by(|a, b| {
            let rank_a = if matches!(a.kind, CloudEntryKind::Dir) {
                0
            } else {
                1
            };
            let rank_b = if matches!(b.kind, CloudEntryKind::Dir) {
                0
            } else {
                1
            };
            rank_a.cmp(&rank_b).then_with(|| a.name.cmp(&b.name))
        });
        Ok(entries)
    }
}

fn map_rclone_error(error: RcloneCliError) -> CloudCommandError {
    match error {
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            CloudCommandError::new(
                CloudCommandErrorCode::BinaryMissing,
                "rclone not found in PATH",
            )
        }
        RcloneCliError::Io(io) => CloudCommandError::new(
            CloudCommandErrorCode::UnknownError,
            format!("Failed to run rclone: {io}"),
        ),
        RcloneCliError::NonZero { stderr, stdout, .. } => {
            let msg = if !stderr.trim().is_empty() {
                stderr
            } else {
                stdout
            };
            let lower = msg.to_ascii_lowercase();
            if lower.contains("didn't find section") || lower.contains("not configured") {
                CloudCommandError::new(CloudCommandErrorCode::InvalidConfig, msg.trim())
            } else {
                CloudCommandError::new(CloudCommandErrorCode::UnknownError, msg.trim())
            }
        }
    }
}

fn parse_listremotes_plain(stdout: &str) -> Result<Vec<String>, String> {
    let mut remotes = Vec::new();
    for raw in stdout.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some(name) = line.strip_suffix(':') else {
            return Err(format!("Unexpected rclone listremotes output line: {line}"));
        };
        if name.is_empty() {
            return Err("Empty remote name in rclone listremotes output".to_string());
        }
        remotes.push(name.to_string());
    }
    Ok(remotes)
}

fn parse_config_dump_types(stdout: &str) -> Result<HashMap<String, String>, String> {
    let value: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("Invalid rclone config dump JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "Expected top-level object from rclone config dump".to_string())?;
    let mut out = HashMap::new();
    for (remote, config) in obj {
        if let Some(ty) = config
            .as_object()
            .and_then(|cfg| cfg.get("type"))
            .and_then(|v| v.as_str())
        {
            out.insert(remote.to_string(), ty.to_ascii_lowercase());
        }
    }
    Ok(out)
}

fn classify_provider_kind(rclone_type: &str) -> Option<CloudProviderKind> {
    match rclone_type.trim().to_ascii_lowercase().as_str() {
        "onedrive" => Some(CloudProviderKind::Onedrive),
        "drive" => Some(CloudProviderKind::Gdrive),
        "nextcloud" => Some(CloudProviderKind::Nextcloud),
        // Nextcloud is commonly configured through rclone's webdav backend; we avoid guessing here.
        _ => None,
    }
}

fn format_remote_label(remote_id: &str, provider: CloudProviderKind) -> String {
    let provider_label = match provider {
        CloudProviderKind::Onedrive => "OneDrive",
        CloudProviderKind::Gdrive => "Google Drive",
        CloudProviderKind::Nextcloud => "Nextcloud",
    };
    format!("{remote_id} ({provider_label})")
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LsJsonItem {
    name: String,
    #[serde(default)]
    is_dir: bool,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    mod_time: Option<String>,
}

fn parse_lsjson_items(stdout: &str) -> Result<Vec<LsJsonItem>, String> {
    serde_json::from_str(stdout).map_err(|e| format!("Invalid rclone lsjson output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        classify_provider_kind, parse_config_dump_types, parse_listremotes_plain,
        parse_lsjson_items,
    };
    use crate::commands::cloud::types::CloudProviderKind;

    #[test]
    fn parses_listremotes_plain_output() {
        let out = parse_listremotes_plain("work:\npersonal:\n\n").expect("parse");
        assert_eq!(out, vec!["work".to_string(), "personal".to_string()]);
    }

    #[test]
    fn parses_config_dump_type_map() {
        let json = r#"{
          "work": {"type":"onedrive","token":"secret"},
          "photos": {"type":"drive"},
          "misc": {"provider":"something"}
        }"#;
        let map = parse_config_dump_types(json).expect("parse json");
        assert_eq!(map.get("work").map(String::as_str), Some("onedrive"));
        assert_eq!(map.get("photos").map(String::as_str), Some("drive"));
        assert!(!map.contains_key("misc"));
    }

    #[test]
    fn classifies_supported_provider_types() {
        assert_eq!(
            classify_provider_kind("onedrive"),
            Some(CloudProviderKind::Onedrive)
        );
        assert_eq!(
            classify_provider_kind("drive"),
            Some(CloudProviderKind::Gdrive)
        );
        assert_eq!(classify_provider_kind("webdav"), None);
    }

    #[test]
    fn parses_lsjson_items() {
        let json = r#"[
          {"Name":"Folder","IsDir":true,"Size":0,"ModTime":"2026-02-25T10:00:00Z"},
          {"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}
        ]"#;
        let items = parse_lsjson_items(json).expect("parse lsjson");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Folder");
        assert!(items[0].is_dir);
        assert_eq!(items[1].name, "note.txt");
        assert_eq!(items[1].size, Some(12));
    }
}

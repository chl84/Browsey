use super::super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    rclone_cli::{RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand},
    types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind, CloudRemote},
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
use tracing::debug;

const MIN_RCLONE_VERSION: (u64, u64, u64) = (1, 67, 0);

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(in crate::commands::cloud) struct RcloneCloudProvider {
    cli: RcloneCli,
}

static RCLONE_RUNTIME_PROBE: OnceLock<Result<(), CloudCommandError>> = OnceLock::new();

#[allow(dead_code)]
impl RcloneCloudProvider {
    pub fn new(cli: RcloneCli) -> Self {
        Self { cli }
    }

    pub fn cli(&self) -> &RcloneCli {
        &self.cli
    }

    fn ensure_runtime_ready(&self) -> CloudCommandResult<()> {
        let result = RCLONE_RUNTIME_PROBE.get_or_init(|| probe_rclone_runtime(&self.cli));
        match result {
            Ok(_) => Ok(()),
            Err(error) => Err(error.clone()),
        }
    }
}

impl CloudProvider for RcloneCloudProvider {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>> {
        self.ensure_runtime_ready()?;
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

    fn stat_path(&self, path: &CloudPath) -> CloudCommandResult<Option<CloudEntry>> {
        self.ensure_runtime_ready()?;
        let spec = RcloneCommandSpec::new(RcloneSubcommand::LsJson)
            .arg("--stat")
            .arg(path.to_rclone_remote_spec());
        match self.cli.run_capture_text(spec) {
            Ok(output) => {
                let item = parse_lsjson_stat_item(&output.stdout).map_err(|message| {
                    CloudCommandError::new(CloudCommandErrorCode::UnknownError, message)
                })?;
                Ok(Some(cloud_entry_from_item(path, item)))
            }
            Err(RcloneCliError::NonZero { stderr, stdout, .. })
                if is_rclone_not_found_text(&stderr, &stdout) =>
            {
                Ok(None)
            }
            Err(error) => Err(map_rclone_error(error)),
        }
    }

    fn list_dir(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
        self.ensure_runtime_ready()?;
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
            entries.push(cloud_entry_from_item(&child_path, item));
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

    fn mkdir(&self, path: &CloudPath) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        self.cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::Mkdir).arg(path.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        Ok(())
    }

    fn delete_file(&self, path: &CloudPath) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        self.cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::DeleteFile)
                    .arg(path.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        Ok(())
    }

    fn delete_dir_recursive(&self, path: &CloudPath) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        self.cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::Purge).arg(path.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        Ok(())
    }

    fn delete_dir_empty(&self, path: &CloudPath) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        self.cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::Rmdir).arg(path.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        Ok(())
    }

    fn move_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        ensure_destination_overwrite_policy(self, src, dst, overwrite)?;
        self.cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::MoveTo)
                    .arg(src.to_rclone_remote_spec())
                    .arg(dst.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        Ok(())
    }

    fn copy_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        ensure_destination_overwrite_policy(self, src, dst, overwrite)?;
        self.cli
            .run_capture_text(
                RcloneCommandSpec::new(RcloneSubcommand::CopyTo)
                    .arg(src.to_rclone_remote_spec())
                    .arg(dst.to_rclone_remote_spec()),
            )
            .map_err(map_rclone_error)?;
        Ok(())
    }
}

fn probe_rclone_runtime(cli: &RcloneCli) -> CloudCommandResult<()> {
    let output = cli
        .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::Version))
        .map_err(map_rclone_error)?;
    let version = parse_rclone_version_stdout(&output.stdout).ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            "Unexpected `rclone version` output; cannot verify rclone runtime",
        )
    })?;
    let numeric = parse_rclone_version_triplet(&version).ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!("Unsupported rclone version format: {version}"),
        )
    })?;
    if numeric < MIN_RCLONE_VERSION {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!(
                "rclone v{version} is too old; Browsey requires rclone v{}.{}.{} or newer",
                MIN_RCLONE_VERSION.0, MIN_RCLONE_VERSION.1, MIN_RCLONE_VERSION.2
            ),
        ));
    }
    debug!(version = %version, "rclone runtime probe succeeded");
    Ok(())
}

fn parse_rclone_version_stdout(stdout: &str) -> Option<String> {
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let version = line.strip_prefix("rclone v")?.trim();
    if version.is_empty() {
        return None;
    }
    Some(version.to_string())
}

fn parse_rclone_version_triplet(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parse_leading_digits(parts.next()?)?;
    let minor = parse_leading_digits(parts.next()?)?;
    let patch = parse_leading_digits(parts.next()?)?;
    Some((major, minor, patch))
}

fn parse_leading_digits(part: &str) -> Option<u64> {
    let digits: String = part.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn ensure_destination_overwrite_policy(
    provider: &impl CloudProvider,
    src: &CloudPath,
    dst: &CloudPath,
    overwrite: bool,
) -> CloudCommandResult<()> {
    if overwrite || src == dst {
        return Ok(());
    }
    if provider.stat_path(dst)?.is_some() {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::DestinationExists,
            format!("Destination already exists: {dst}"),
        ));
    }
    Ok(())
}

fn cloud_entry_from_item(path: &CloudPath, item: LsJsonItem) -> CloudEntry {
    CloudEntry {
        name: item.name,
        path: path.to_string(),
        kind: if item.is_dir {
            CloudEntryKind::Dir
        } else {
            CloudEntryKind::File
        },
        size: if item.is_dir { None } else { item.size },
        modified: item.mod_time,
        capabilities: CloudCapabilities::v1_core_rw(),
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
            CloudCommandErrorCode::NetworkError,
            format!("Failed to run rclone: {io}"),
        ),
        RcloneCliError::Timeout {
            subcommand,
            timeout,
            ..
        } => CloudCommandError::new(
            CloudCommandErrorCode::Timeout,
            format!(
                "rclone {} timed out after {}s",
                subcommand.as_str(),
                timeout.as_secs()
            ),
        ),
        RcloneCliError::NonZero { stderr, stdout, .. } => {
            let msg = if !stderr.trim().is_empty() {
                stderr
            } else {
                stdout
            };
            let code = classify_rclone_message_code(&msg);
            CloudCommandError::new(code, msg.trim())
        }
    }
}

fn classify_rclone_message_code(message: &str) -> CloudCommandErrorCode {
    let lower = message.to_ascii_lowercase();
    if lower.contains("didn't find section") || lower.contains("not configured") {
        return CloudCommandErrorCode::InvalidConfig;
    }
    if lower.contains("already exists")
        || lower.contains("duplicate object")
        || lower.contains("destination exists")
    {
        return CloudCommandErrorCode::DestinationExists;
    }
    if lower.contains("permission denied") || lower.contains("access denied") {
        return CloudCommandErrorCode::PermissionDenied;
    }
    if lower.contains("too many requests")
        || lower.contains("rate limit")
        || lower.contains("rate-limited")
        || lower.contains("retry after")
        || lower.contains("status code 429")
        || lower.contains("http error 429")
    {
        return CloudCommandErrorCode::RateLimited;
    }
    if lower.contains("unauthorized")
        || lower.contains("authentication failed")
        || lower.contains("login required")
        || lower.contains("invalid_grant")
        || lower.contains("token expired")
        || lower.contains("expired token")
        || lower.contains("status code 401")
        || lower.contains("http error 401")
    {
        return CloudCommandErrorCode::AuthRequired;
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return CloudCommandErrorCode::Timeout;
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dial tcp")
        || lower.contains("no such host")
        || lower.contains("name resolution")
        || lower.contains("tls handshake")
    {
        return CloudCommandErrorCode::NetworkError;
    }
    if is_rclone_not_found_text(message, "") {
        return CloudCommandErrorCode::NotFound;
    }
    CloudCommandErrorCode::UnknownError
}

fn is_rclone_not_found_text(stderr: &str, stdout: &str) -> bool {
    let hay = if !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let lower = hay.to_ascii_lowercase();
    lower.contains("not found")
        || lower.contains("object not found")
        || lower.contains("directory not found")
        || lower.contains("file not found")
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

fn parse_lsjson_stat_item(stdout: &str) -> Result<LsJsonItem, String> {
    serde_json::from_str(stdout).map_err(|e| format!("Invalid rclone lsjson --stat output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        classify_provider_kind, classify_rclone_message_code, is_rclone_not_found_text,
        map_rclone_error, parse_config_dump_types, parse_listremotes_plain, parse_lsjson_items,
        parse_lsjson_stat_item, parse_rclone_version_stdout, parse_rclone_version_triplet,
    };
    use crate::{
        commands::cloud::{
            error::CloudCommandErrorCode, rclone_cli::RcloneCliError, rclone_cli::RcloneSubcommand,
            types::CloudProviderKind,
        },
        errors::domain::{DomainError, ErrorCode},
    };
    use std::{process::ExitStatus, time::Duration};

    #[cfg(unix)]
    fn fake_exit_status(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn fake_exit_status(code: u32) -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code)
    }

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

    #[test]
    fn parses_lsjson_stat_item() {
        let json =
            r#"{"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}"#;
        let item = parse_lsjson_stat_item(json).expect("parse lsjson stat");
        assert_eq!(item.name, "note.txt");
        assert!(!item.is_dir);
        assert_eq!(item.size, Some(12));
    }

    #[test]
    fn detects_not_found_rclone_messages() {
        assert!(is_rclone_not_found_text(
            "Failed to lsjson: object not found",
            ""
        ));
        assert!(is_rclone_not_found_text("", "directory not found"));
        assert!(!is_rclone_not_found_text("permission denied", ""));
    }

    #[test]
    fn classifies_common_rclone_error_messages() {
        assert_eq!(
            classify_rclone_message_code("Failed to move: destination exists"),
            CloudCommandErrorCode::DestinationExists
        );
        assert_eq!(
            classify_rclone_message_code("Permission denied"),
            CloudCommandErrorCode::PermissionDenied
        );
        assert_eq!(
            classify_rclone_message_code("object not found"),
            CloudCommandErrorCode::NotFound
        );
        assert_eq!(
            classify_rclone_message_code("HTTP error 429: too many requests"),
            CloudCommandErrorCode::RateLimited
        );
        assert_eq!(
            classify_rclone_message_code("authentication failed: token expired"),
            CloudCommandErrorCode::AuthRequired
        );
        assert_eq!(
            classify_rclone_message_code("dial tcp: i/o timeout"),
            CloudCommandErrorCode::Timeout
        );
    }

    #[test]
    fn parses_rclone_version_output() {
        let out = "rclone v1.69.1\n- os/version: fedora 41\n";
        assert_eq!(parse_rclone_version_stdout(out).as_deref(), Some("1.69.1"));
        assert_eq!(parse_rclone_version_stdout("not-rclone\n"), None);
    }

    #[test]
    fn parses_rclone_version_triplet_with_suffixes() {
        assert_eq!(parse_rclone_version_triplet("1.69.1"), Some((1, 69, 1)));
        assert_eq!(
            parse_rclone_version_triplet("1.68.0-beta.1"),
            Some((1, 68, 0))
        );
        assert_eq!(parse_rclone_version_triplet("v1.69.1"), None);
        assert_eq!(parse_rclone_version_triplet("1.69"), None);
    }

    #[test]
    fn maps_rclone_timeout_to_cloud_timeout_error_code() {
        let err = map_rclone_error(RcloneCliError::Timeout {
            subcommand: RcloneSubcommand::CopyTo,
            timeout: Duration::from_secs(10),
            stdout: String::new(),
            stderr: "timed out".to_string(),
        });
        assert_eq!(err.code_str(), CloudCommandErrorCode::Timeout.as_code_str());
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn maps_rclone_nonzero_stderr_to_cloud_error_code() {
        let err = map_rclone_error(RcloneCliError::NonZero {
            status: fake_exit_status(1),
            stdout: String::new(),
            stderr: "HTTP error 429: too many requests".to_string(),
        });
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::RateLimited.as_code_str()
        );
    }
}

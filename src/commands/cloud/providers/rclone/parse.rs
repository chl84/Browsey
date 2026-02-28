use super::{CloudProviderKind, HashMap, Value};

pub(super) fn parse_listremotes_plain(stdout: &str) -> Result<Vec<String>, String> {
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

pub(super) fn parse_listremotes_rc_json(value: &Value) -> Result<Vec<String>, String> {
    let remotes = value
        .get("remotes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "Missing `remotes` array in rclone rc config/listremotes output".to_string()
        })?;
    let mut out = Vec::new();
    for entry in remotes {
        let Some(name) = entry.as_str() else {
            return Err(
                "Non-string remote entry in rclone rc config/listremotes output".to_string(),
            );
        };
        if name.trim().is_empty() {
            return Err("Empty remote name in rclone rc config/listremotes output".to_string());
        }
        out.push(name.to_string());
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RcloneRemoteConfigSummary {
    pub(super) backend_type: String,
    pub(super) vendor: Option<String>,
    pub(super) url: Option<String>,
    pub(super) has_password: bool,
}

pub(super) fn parse_config_dump_summaries(
    stdout: &str,
) -> Result<HashMap<String, RcloneRemoteConfigSummary>, String> {
    let value: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("Invalid rclone config dump JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "Expected top-level object from rclone config dump".to_string())?;
    let mut out = HashMap::new();
    for (remote, config) in obj {
        let Some(cfg) = config.as_object() else {
            continue;
        };
        let Some(backend_type) = cfg.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        let vendor = cfg
            .get("vendor")
            .and_then(|v| v.as_str())
            .map(|v| v.to_ascii_lowercase());
        let url = cfg
            .get("url")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let has_password = cfg.contains_key("pass") || cfg.contains_key("password");
        out.insert(
            remote.to_string(),
            RcloneRemoteConfigSummary {
                backend_type: backend_type.to_ascii_lowercase(),
                vendor,
                url,
                has_password,
            },
        );
    }
    Ok(out)
}

pub(super) fn classify_provider_kind(rclone_type: &str) -> Option<CloudProviderKind> {
    match rclone_type.trim().to_ascii_lowercase().as_str() {
        "onedrive" => Some(CloudProviderKind::Onedrive),
        "drive" => Some(CloudProviderKind::Gdrive),
        "nextcloud" => Some(CloudProviderKind::Nextcloud),
        _ => None,
    }
}

pub(super) fn classify_provider_kind_from_config(
    cfg: &RcloneRemoteConfigSummary,
) -> Option<CloudProviderKind> {
    if let Some(kind) = classify_provider_kind(&cfg.backend_type) {
        return Some(kind);
    }
    if cfg.backend_type == "webdav" {
        if cfg.vendor.as_deref() == Some("nextcloud") {
            return Some(CloudProviderKind::Nextcloud);
        }
        if cfg
            .url
            .as_deref()
            .map(|url| url.to_ascii_lowercase().contains("nextcloud"))
            .unwrap_or(false)
        {
            return Some(CloudProviderKind::Nextcloud);
        }
    }
    None
}

pub(super) fn parse_rclone_version_stdout(stdout: &str) -> Option<String> {
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

pub(super) fn parse_rclone_version_triplet(version: &str) -> Option<(u64, u64, u64)> {
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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct LsJsonItem {
    pub(super) name: String,
    #[serde(default)]
    pub(super) is_dir: bool,
    #[serde(default, deserialize_with = "deserialize_lsjson_size")]
    pub(super) size: Option<u64>,
    #[serde(default)]
    pub(super) mod_time: Option<String>,
}

fn deserialize_lsjson_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <Option<i64> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(raw.and_then(|n| u64::try_from(n).ok()))
}

pub(super) fn parse_lsjson_items(stdout: &str) -> Result<Vec<LsJsonItem>, String> {
    serde_json::from_str(stdout).map_err(|e| format!("Invalid rclone lsjson output: {e}"))
}

pub(super) fn parse_lsjson_stat_item(stdout: &str) -> Result<LsJsonItem, String> {
    serde_json::from_str(stdout).map_err(|e| format!("Invalid rclone lsjson --stat output: {e}"))
}

pub(super) fn parse_lsjson_items_value(value: Value) -> Result<Vec<LsJsonItem>, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid rclone lsjson output: {e}"))
}

pub(super) fn parse_lsjson_stat_item_value(value: Value) -> Result<LsJsonItem, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid rclone lsjson --stat output: {e}"))
}

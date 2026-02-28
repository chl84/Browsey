use super::{
    error::map_rclone_error,
    logging::{classify_rc_fallback_reason, log_backend_selected},
    parse::{
        classify_provider_kind_from_config, parse_config_dump_summaries,
        parse_config_dump_summaries_value, parse_listremotes_plain, parse_listremotes_rc_json,
        RcloneRemoteConfigSummary,
    },
    CloudCapabilities, CloudCommandError, CloudCommandErrorCode, CloudCommandResult,
    CloudProviderKind, CloudRemote, RcloneCliError, RcloneCloudProvider, RcloneCommandSpec,
    RcloneSubcommand,
};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::OnceLock,
};
use tracing::info;

static RCLONE_REMOTE_POLICY: OnceLock<RcloneRemotePolicy> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub(super) struct RcloneRemotePolicy {
    pub(super) allowlist: Option<HashSet<String>>,
    pub(super) prefix: Option<String>,
}

impl RcloneCloudProvider {
    pub(super) fn list_remotes_impl(&self) -> CloudCommandResult<Vec<CloudRemote>> {
        self.ensure_runtime_ready()?;
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_read_enabled() {
            match self.list_remotes_via_rc() {
                Ok(remotes) => {
                    log_backend_selected("cloud_list_remotes", "rc", false, None);
                    return Ok(remotes);
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    info!(
                        error = %error,
                        "rclone rc remote discovery failed; falling back to CLI listremotes"
                    );
                }
            }
        }
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
        let config_map = parse_config_dump_summaries_bytes(config_dump.stdout.into_bytes())
            .map_err(|message| {
                CloudCommandError::new(CloudCommandErrorCode::InvalidConfig, message)
            })?;
        log_backend_selected(
            "cloud_list_remotes",
            "cli",
            fell_back_from_rc,
            fallback_reason,
        );
        Ok(build_cloud_remotes(remote_ids, config_map))
    }

    fn list_remotes_via_rc(&self) -> Result<Vec<CloudRemote>, RcloneCliError> {
        let remotes_value = self.rc.list_remotes()?;
        let remote_ids = parse_listremotes_rc_json(&remotes_value)
            .map_err(|msg| RcloneCliError::Io(std::io::Error::other(msg)))?;
        let config_dump_value = self.rc.config_dump()?;
        let config_map = parse_config_dump_summaries_value(config_dump_value).map_err(|msg| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc config dump payload: {msg}"
            )))
        })?;
        Ok(build_cloud_remotes(remote_ids, config_map))
    }
}

pub(super) fn remote_allowed_by_policy_with(policy: &RcloneRemotePolicy, remote_id: &str) -> bool {
    if let Some(allowlist) = &policy.allowlist {
        if !allowlist.contains(remote_id) {
            return false;
        }
    }
    if let Some(prefix) = &policy.prefix {
        if !remote_id.starts_with(prefix) {
            return false;
        }
    }
    true
}

fn remote_allowed_by_policy(remote_id: &str) -> bool {
    let policy = RCLONE_REMOTE_POLICY.get_or_init(load_remote_policy_from_env);
    remote_allowed_by_policy_with(policy, remote_id)
}

fn load_remote_policy_from_env() -> RcloneRemotePolicy {
    let allowlist = env::var("BROWSEY_RCLONE_REMOTE_ALLOWLIST")
        .ok()
        .and_then(|raw| {
            let set = raw
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect::<HashSet<_>>();
            if set.is_empty() {
                None
            } else {
                Some(set)
            }
        });
    let prefix = env::var("BROWSEY_RCLONE_REMOTE_PREFIX")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    RcloneRemotePolicy { allowlist, prefix }
}

fn format_remote_label(remote_id: &str, provider: CloudProviderKind) -> String {
    let provider_label = match provider {
        CloudProviderKind::Onedrive => "OneDrive",
        CloudProviderKind::Gdrive => "Google Drive",
        CloudProviderKind::Nextcloud => "Nextcloud",
    };
    format!("{remote_id} ({provider_label})")
}

fn build_cloud_remotes(
    remote_ids: Vec<String>,
    config_map: HashMap<String, RcloneRemoteConfigSummary>,
) -> Vec<CloudRemote> {
    let mut remotes = Vec::new();
    let mut seen = HashSet::new();
    for remote_id in remote_ids {
        if !seen.insert(remote_id.clone()) {
            continue;
        }
        if !remote_allowed_by_policy(&remote_id) {
            continue;
        }
        let Some(provider) = config_map
            .get(&remote_id)
            .and_then(classify_provider_kind_from_config)
        else {
            continue;
        };
        remotes.push(CloudRemote {
            id: remote_id.clone(),
            label: format_remote_label(&remote_id, provider),
            provider,
            root_path: format!("rclone://{remote_id}"),
            capabilities: CloudCapabilities::v1_for_provider(provider),
        });
    }
    remotes.sort_by(|a, b| a.label.cmp(&b.label));
    remotes
}

fn parse_config_dump_summaries_bytes(
    mut stdout: Vec<u8>,
) -> Result<HashMap<String, RcloneRemoteConfigSummary>, String> {
    let parse_result = std::str::from_utf8(&stdout)
        .map_err(|error| format!("Invalid UTF-8 in rclone config dump output: {error}"))
        .and_then(parse_config_dump_summaries);
    stdout.fill(0);
    parse_result
}

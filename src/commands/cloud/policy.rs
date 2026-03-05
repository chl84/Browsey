use super::{error::CloudCommandErrorCode, types::CloudProviderKind};

const ONEDRIVE_DELETE_POLICY_ARGS: &[&str] = &["--onedrive-hard-delete"];
const GDRIVE_DELETE_POLICY_ARGS: &[&str] = &["--drive-use-trash=false"];
const NEXTCLOUD_DELETE_POLICY_ARGS: &[&str] = &[];
const MKDIR_DESTINATION_EXISTS_RETRY_BACKOFFS_MS: &[u64] = &[75, 200, 500];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderPolicy {
    /// Extra args appended to destructive delete commands for this provider.
    pub(crate) delete_policy_args: &'static [&'static str],
    /// Whether conflict-key comparisons should normalize name casing.
    pub(crate) conflict_case_insensitive: bool,
}

/// Returns the stable provider-policy contract used by shared cloud write/conflict flows.
pub(crate) fn provider_policy(kind: CloudProviderKind) -> ProviderPolicy {
    // Provider policy baseline is locked by:
    // - rclone tests:
    //   `delete_ops_use_cli_delete_policy_flags`
    //   `delete_ops_use_gdrive_cli_delete_policy_flags`
    //   `delete_ops_use_nextcloud_default_policy_without_provider_flags`
    // - conflict tests:
    //   `conflict_preview_is_case_insensitive_for_onedrive_names`
    //   `conflict_preview_stays_case_sensitive_for_non_onedrive_names`
    match kind {
        CloudProviderKind::Onedrive => ProviderPolicy {
            delete_policy_args: ONEDRIVE_DELETE_POLICY_ARGS,
            conflict_case_insensitive: true,
        },
        CloudProviderKind::Gdrive => ProviderPolicy {
            delete_policy_args: GDRIVE_DELETE_POLICY_ARGS,
            conflict_case_insensitive: false,
        },
        CloudProviderKind::Nextcloud => ProviderPolicy {
            delete_policy_args: NEXTCLOUD_DELETE_POLICY_ARGS,
            conflict_case_insensitive: false,
        },
    }
}

/// Delete-policy command args for the given provider.
pub(crate) fn cloud_delete_policy_args(kind: CloudProviderKind) -> &'static [&'static str] {
    provider_policy(kind).delete_policy_args
}

/// Retry backoff windows used when `mkdir` reports transient `destination_exists`.
///
/// This remains provider-tunable through the hook signature even when values are shared.
pub(crate) fn mkdir_destination_exists_retry_backoffs_ms(
    _provider: Option<CloudProviderKind>,
) -> &'static [u64] {
    // Hook point for provider tuning. Current baseline keeps identical backoff windows.
    MKDIR_DESTINATION_EXISTS_RETRY_BACKOFFS_MS
}

/// Optional provider-specific error hinting layered on top of common rclone classification.
pub(crate) fn classify_provider_rclone_message_code(
    provider: CloudProviderKind,
    message: &str,
) -> Option<CloudCommandErrorCode> {
    let lower = message.to_ascii_lowercase();
    match provider {
        CloudProviderKind::Onedrive => {
            if lower.contains("activitylimitreached") {
                return Some(CloudCommandErrorCode::RateLimited);
            }
            None
        }
        CloudProviderKind::Gdrive => None,
        CloudProviderKind::Nextcloud => None,
    }
}

/// Conflict key used by cross-provider name conflict previews.
pub(crate) fn cloud_conflict_name_key(provider: Option<CloudProviderKind>, name: &str) -> String {
    match provider {
        Some(kind) if provider_policy(kind).conflict_case_insensitive => name.to_lowercase(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_provider_rclone_message_code, cloud_conflict_name_key, cloud_delete_policy_args,
        mkdir_destination_exists_retry_backoffs_ms,
    };
    use crate::commands::cloud::{error::CloudCommandErrorCode, types::CloudProviderKind};

    #[test]
    fn delete_policy_args_are_provider_specific() {
        assert_eq!(
            cloud_delete_policy_args(CloudProviderKind::Onedrive),
            &["--onedrive-hard-delete"]
        );
        assert_eq!(
            cloud_delete_policy_args(CloudProviderKind::Gdrive),
            &["--drive-use-trash=false"]
        );
        assert_eq!(
            cloud_delete_policy_args(CloudProviderKind::Nextcloud),
            &[] as &[&str]
        );
    }

    #[test]
    fn conflict_key_casing_is_provider_specific() {
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Onedrive), "Report.TXT"),
            "report.txt"
        );
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Onedrive), "RÄPORT.TXT"),
            "räport.txt"
        );
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Gdrive), "Report.TXT"),
            "Report.TXT"
        );
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Nextcloud), "Report.TXT"),
            "Report.TXT"
        );
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Gdrive), "RÄPORT.TXT"),
            "RÄPORT.TXT"
        );
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Nextcloud), "RÄPORT.TXT"),
            "RÄPORT.TXT"
        );
    }

    #[test]
    fn provider_specific_error_hints_are_isolated() {
        assert_eq!(
            classify_provider_rclone_message_code(
                CloudProviderKind::Onedrive,
                "graph returned ActivityLimitReached"
            ),
            Some(CloudCommandErrorCode::RateLimited)
        );
        assert_eq!(
            classify_provider_rclone_message_code(
                CloudProviderKind::Gdrive,
                "graph returned ActivityLimitReached"
            ),
            None
        );
        assert_eq!(
            classify_provider_rclone_message_code(
                CloudProviderKind::Nextcloud,
                "graph returned ActivityLimitReached"
            ),
            None
        );
    }

    #[test]
    fn mkdir_destination_exists_backoff_is_exposed_by_policy_hook() {
        let expected = &[75, 200, 500];
        assert_eq!(
            mkdir_destination_exists_retry_backoffs_ms(Some(CloudProviderKind::Onedrive)),
            expected
        );
        assert_eq!(
            mkdir_destination_exists_retry_backoffs_ms(Some(CloudProviderKind::Gdrive)),
            expected
        );
        assert_eq!(
            mkdir_destination_exists_retry_backoffs_ms(Some(CloudProviderKind::Nextcloud)),
            expected
        );
        assert_eq!(mkdir_destination_exists_retry_backoffs_ms(None), expected);
    }
}

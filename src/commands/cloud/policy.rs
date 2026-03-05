use super::types::CloudProviderKind;

const ONEDRIVE_DELETE_POLICY_ARGS: &[&str] = &["--onedrive-hard-delete"];
const GDRIVE_DELETE_POLICY_ARGS: &[&str] = &["--drive-use-trash=false"];
const NEXTCLOUD_DELETE_POLICY_ARGS: &[&str] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderPolicy {
    pub(crate) delete_policy_args: &'static [&'static str],
    pub(crate) conflict_case_insensitive: bool,
}

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

pub(crate) fn cloud_delete_policy_args(kind: CloudProviderKind) -> &'static [&'static str] {
    provider_policy(kind).delete_policy_args
}

pub(crate) fn cloud_conflict_name_key(provider: Option<CloudProviderKind>, name: &str) -> String {
    match provider {
        Some(kind) if provider_policy(kind).conflict_case_insensitive => name.to_ascii_lowercase(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{cloud_conflict_name_key, cloud_delete_policy_args};
    use crate::commands::cloud::types::CloudProviderKind;

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
            cloud_conflict_name_key(Some(CloudProviderKind::Gdrive), "Report.TXT"),
            "Report.TXT"
        );
        assert_eq!(
            cloud_conflict_name_key(Some(CloudProviderKind::Nextcloud), "Report.TXT"),
            "Report.TXT"
        );
    }
}

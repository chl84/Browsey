//! Shared opt-in ownership guard. Failure retains fixtures; cleanup is explicit.
use super::{
    path::CloudPath,
    provider::CloudProvider,
    providers::rclone::RcloneCloudProvider,
    rclone_cli::RcloneCli,
    types::{CloudEntryKind, CloudProviderKind},
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct OneDriveFixture {
    pub(crate) local: PathBuf,
    pub(crate) child: CloudPath,
    pub(crate) provider: RcloneCloudProvider,
    parent: CloudPath,
    owner: String,
    _daemon: TestDaemon,
}

struct TestDaemon;
impl Drop for TestDaemon {
    fn drop(&mut self) {
        // This is the test process's own RC daemon, not the installed app's.
        let _ = super::rclone_rc::begin_shutdown_and_kill_daemon();
    }
}

fn approved_scope(raw: &str, approved: &str) -> Result<CloudPath, Box<dyn std::error::Error>> {
    let path = CloudPath::parse(raw)?;
    if path.is_root() || approved != "yes" {
        return Err("Explicit approval and a non-root disposable folder are required".into());
    }
    Ok(path)
}

impl OneDriveFixture {
    pub(crate) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Own cleanup even when initialization fails before a fixture exists.
        let daemon = TestDaemon;
        let parent = approved_scope(
            &std::env::var("BROWSEY_TEST_CLOUD_SCOPE")?,
            &std::env::var("BROWSEY_TEST_CLOUD_WRITE_APPROVED")?,
        )?;
        let provider = RcloneCloudProvider::new(RcloneCli::new("rclone"));
        if !provider.list_remotes()?.iter().any(|remote| {
            remote.id == parent.remote() && remote.provider == CloudProviderKind::Onedrive
        }) {
            return Err("The approved remote must be a supported OneDrive remote".into());
        }
        if !provider
            .stat_path(&parent)?
            .is_some_and(|entry| matches!(entry.kind, CloudEntryKind::Dir))
            || !provider.list_dir(&parent)?.is_empty()
        {
            return Err(
                "The approved test folder must exist and be empty; no writes performed".into(),
            );
        }
        let owner = format!(
            "browsey-cloud-acceptance-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let local = std::env::temp_dir().join(&owner);
        fs::create_dir(&local)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Fault diagnostics can contain provider-signed upload URLs.
            fs::set_permissions(&local, fs::Permissions::from_mode(0o700))?;
        }
        let child = parent.child_path(&owner)?;
        if provider.stat_path(&child)?.is_some() {
            return Err("Refusing to reuse an existing test child".into());
        }
        eprintln!(
            "Owned disposable acceptance child: {child}; local data: {}",
            local.display()
        );
        provider.mkdir(&child, None)?;
        let marker = local.join("owner.txt");
        fs::write(&marker, &owner)?;
        provider.upload_new_file(&marker, &child.child_path("browsey-test-owner.txt")?, None)?;
        Ok(Self {
            local,
            child,
            provider,
            parent,
            owner,
            _daemon: daemon,
        })
    }

    pub(crate) fn finish(self) -> Result<(), Box<dyn std::error::Error>> {
        let marker = self.local.join("cleanup-owner.txt");
        self.provider.download_file(
            &self.child.child_path("browsey-test-owner.txt")?,
            &marker,
            None,
        )?;
        if fs::read_to_string(marker)? != self.owner {
            return Err("Ownership marker changed; refusing cloud cleanup".into());
        }
        self.provider.trash_entry(&self.child, None)?;
        assert!(self.provider.stat_path(&self.child)?.is_none());
        assert!(self.provider.list_dir(&self.parent)?.is_empty());
        fs::remove_dir_all(&self.local)?;
        eprintln!("PASS: verified owned child moved to normal trash; parent empty. Web restore not verified.");
        Ok(())
    }
}

#[test]
fn real_cloud_scope_requires_explicit_approval_and_refuses_remote_root() {
    assert!(approved_scope("rclone://Onedrive/", "yes").is_err());
    assert!(approved_scope("rclone://Onedrive/test", "no").is_err());
    assert!(approved_scope("rclone://Onedrive/test", "yes").is_ok());
}

use super::*;
use crate::commands::cloud::{acceptance_tests::OneDriveFixture, provider::CloudProvider};
use crate::errors::domain::DomainError;
use std::os::unix::fs::PermissionsExt;

#[test]
#[ignore = "Adds a competing test-only destination after preflight in an approved OneDrive fixture"]
fn real_onedrive_post_preflight_destination_acceptance() -> Result<(), Box<dyn std::error::Error>> {
    let scope = OneDriveFixture::new()?;
    let source = scope.local.join("source.txt");
    let competitor = scope.local.join("competing-writer.txt");
    fs::write(&source, b"our payload A")?;
    fs::write(&competitor, b"our payload B")?;
    let target = scope.child.child_path("raced-target.txt")?;
    let real = which::which("rclone")?;
    let notified = scope.local.join("copy-starting");
    let release = scope.local.join("release-copy");
    let quoted = |path: &Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    let wrapper = scope.local.join("gated-rclone.sh");
    fs::write(&wrapper, format!("#!/usr/bin/env bash\nset -euo pipefail\nfor arg in \"$@\"; do\n if [[ \"$arg\" == copyto ]]; then\n  touch {}\n  for ((i=0;i<1000;i++)); do [[ -f {} ]] && break; sleep 0.05; done\n  [[ -f {} ]] || exit 1\n fi\ndone\nexec {} \"$@\"\n", quoted(&notified), quoted(&release), quoted(&release), quoted(&real)))?;
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700))?;
    let cli = RcloneCli::new(wrapper.as_os_str());
    let cancel = Arc::new(AtomicBool::new(false));
    let token = cancel.clone();
    let pair = MixedTransferPair {
        src: LocalOrCloudArg::Local(source.clone()),
        dst: LocalOrCloudArg::Cloud(target.clone()),
        cloud_remote_for_error_mapping: Some(scope.child.remote().into()),
    };
    let worker = thread::spawn(move || {
        execute_mixed_entry_to_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            pair,
            MixedTransferWriteOptions::default(),
            Some(token),
            None,
        )
    });
    let started = Instant::now();
    while !notified.exists() && !worker.is_finished() && started.elapsed() < Duration::from_secs(30)
    {
        thread::sleep(Duration::from_millis(25));
    }
    if !notified.exists() {
        cancel.store(true, Ordering::SeqCst);
        let _ = worker.join();
        return Err("No post-preflight boundary observed".into());
    }
    let competing_upload = scope.provider.upload_new_file(&competitor, &target, None);
    if let Err(error) = fs::write(release, b"release owned copy") {
        cancel.store(true, Ordering::SeqCst);
        let _ = worker.join();
        return Err(error.into());
    }
    let result = worker.join().map_err(|_| "Owned copy worker panicked")?;
    competing_upload?;
    assert_eq!(
        result
            .expect_err("Must not overwrite a newly appeared destination")
            .code_str(),
        "task_failed"
    );
    let returned = scope.local.join("competing-data-returned.txt");
    scope.provider.download_file(&target, &returned, None)?;
    assert_eq!(fs::read(returned)?, b"our payload B");
    // Exercise the production progress/RC payload against the same competing
    // object too. Its per-call protection must not disappear when progress is on.
    let rc = crate::commands::cloud::rclone_rc::RcloneRcClient::with_binary(real.as_os_str());
    let before = rc.core_stats(Some("owned-race-acceptance"), true)?["transfers"]
        .as_u64()
        .ok_or("Missing RC transfer baseline")?;
    rc.operations_copyfile_from_local_with_progress(
        crate::commands::cloud::rclone_rc::RcCopyFileFromLocalProgressSpec {
            src_dir: scope.local.to_str().ok_or("Non-UTF8 test directory")?,
            src_remote: "source.txt",
            dst_fs: &format!("{}:", scope.child.remote()),
            dst_remote: target.rel_path(),
            group: "owned-race-acceptance",
            cancel_token: None,
            refuse_replace: true,
        },
        |_| {},
    )?;
    let stats = rc.core_stats(Some("owned-race-acceptance"), true)?;
    let error = crate::commands::cloud::providers::rclone::verify_new_rc_transfer(before, &stats)
        .expect_err("RC skip must not be reported as a successful upload");
    assert_eq!(error.code_str(), "task_failed");
    let returned = scope.local.join("after-rc.txt");
    scope.provider.download_file(&target, &returned, None)?;
    assert_eq!(fs::read(returned)?, b"our payload B");
    assert_eq!(fs::read(source)?, b"our payload A");
    let empty = scope.local.join("empty-source.txt");
    fs::write(&empty, b"")?;
    let empty_target = scope.child.child_path("new-empty.txt")?;
    scope.provider.upload_new_file_with_progress(
        &empty,
        &empty_target,
        "owned-empty-acceptance",
        None,
        |_, _| {},
    )?;
    assert_eq!(
        scope
            .provider
            .stat_path(&empty_target)?
            .ok_or("New empty file missing")?
            .size,
        Some(0)
    );
    let returned_empty = scope.local.join("returned-empty.txt");
    scope
        .provider
        .download_file(&empty_target, &returned_empty, None)?;
    assert!(fs::read(returned_empty)?.is_empty());
    eprintln!("PASS: same-size OneDrive destination created after Browsey preflight refused; RC progress protection also refused replacement; competing/source bytes retained. Not provider CAS or in-flight race proof.");
    eprintln!("PASS: legitimate zero-byte RC upload reports success and exists remotely");
    scope.finish()?;
    Ok(())
}

//! Opt-in end-to-end candidate UI, real IPC, staging and OneDrive verification.
use super::*;
use crate::commands::cloud::{acceptance_tests::OneDriveFixture, provider::CloudProvider};
use std::process::{Child, Command, Stdio};

struct Candidate {
    child: Child,
    executable: PathBuf,
    data: PathBuf,
    observer: PathBuf,
}
impl Drop for Candidate {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            // Best-effort normal exit also stops the private RC daemon.
            let _ = Command::new("/usr/bin/python")
                .arg(&self.observer)
                .arg(self.child.id().to_string())
                .arg(&self.executable)
                .arg(&self.data)
                .args(["click", "button", "Close window"])
                .env("BROWSEY_TEST_A11Y_TIMEOUT", "3")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let deadline = Instant::now() + Duration::from_secs(3);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
            }
            if self.child.try_wait().ok().flatten().is_none() {
                // On failure retain the fixture, but don't orphan our RC child.
                if let Ok(output) = Command::new("ps").args(["-eo", "pid=,ppid="]).output() {
                    for line in String::from_utf8_lossy(&output.stdout).lines() {
                        let fields = line.split_whitespace().collect::<Vec<_>>();
                        if fields.len() != 2
                            || fields[1].parse::<u32>().ok() != Some(self.child.id())
                        {
                            continue;
                        }
                        let Some(pid) = fields[0].parse::<u32>().ok() else {
                            continue;
                        };
                        let proc = PathBuf::from(format!("/proc/{pid}"));
                        let private = fs::read(proc.join("environ")).ok().is_some_and(|bytes| {
                            bytes.split(|byte| *byte == 0).any(|value| {
                                value == format!("XDG_DATA_HOME={}", self.data.display()).as_bytes()
                            })
                        });
                        let rclone = fs::read_link(proc.join("exe")).ok().is_some_and(|path| {
                            path.file_name().is_some_and(|name| name == "rclone")
                        });
                        if private && rclone {
                            let _ = Command::new("kill")
                                .args(["-TERM", &pid.to_string()])
                                .status();
                        }
                    }
                }
                let _ = self.child.kill();
            }
        }
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "Opens a private native candidate on Hyprland and writes only an approved OneDrive fixture"]
fn real_onedrive_native_archive_acceptance() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        std::env::var("BROWSEY_NATIVE_CLOUD_TEST_APPROVED")?.as_str(),
        "yes"
    );
    assert!(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let executable = repo.join("target/release/browsey");
    assert_eq!(
        fs::canonicalize(&executable)?,
        executable,
        "Do not redirect to the installed app"
    );
    let config = Command::new(which::which("rclone")?)
        .args(["config", "file"])
        .output()?;
    assert!(config.status.success());
    let config = String::from_utf8(config.stdout)?;
    let config = PathBuf::from(
        config
            .lines()
            .last()
            .ok_or("Missing normal rclone config path")?,
    );
    assert!(config.is_file()); // Read auth in place; never copy or print it.
    let scope = OneDriveFixture::new()?;
    let inputs = scope.local.join("inputs");
    let folder = inputs.join("native-folder");
    fs::create_dir_all(folder.join("empty/nested"))?;
    fs::write(folder.join("content.txt"), b"native UI cloud acceptance\n")?;
    let source = scope.child.child_path("native-folder")?;
    execute_mixed_entry_to_blocking_with_cli(
        scope.provider.cli(),
        MixedTransferOp::Copy,
        MixedTransferPair {
            src: LocalOrCloudArg::Local(folder),
            dst: LocalOrCloudArg::Cloud(source.clone()),
            cloud_remote_for_error_mapping: Some(scope.child.remote().into()),
        },
        MixedTransferWriteOptions::default(),
        None,
        None,
    )?;
    let profile = scope.local.join("profile");
    let data = profile.join("data");
    fs::create_dir_all(data.join("browsey"))?;
    {
        // Seed only our disposable profile using the production Settings schema.
        let connection = rusqlite::Connection::open(data.join("browsey/browsey.db"))?;
        connection
            .execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        crate::db::set_setting_bool(&connection, "cloudEnabled", true)?;
        crate::db::set_setting_string(&connection, "startDir", &scope.child.to_string())?;
    }
    let log = fs::File::create(scope.local.join("candidate.log"))?;
    let child = Command::new(&executable)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", profile.join("config"))
        .env("XDG_CACHE_HOME", profile.join("cache"))
        .env("XDG_STATE_HOME", profile.join("state"))
        .env("BROWSEY_UNDO_DIR", data.join("browsey/undo-sessions"))
        .env("NO_AT_BRIDGE", "0")
        .env("RCLONE_CONFIG", config)
        .env("RUST_LOG", "warn")
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log))
        .spawn()?;
    let mut candidate = Candidate {
        child,
        executable: executable.clone(),
        data: data.clone(),
        observer: repo.join("tests/support/native_fixture_a11y.py"),
    };
    let pid = candidate.child.id().to_string();
    let action = |command: &str,
                  role: &str,
                  name: &str,
                  value: Option<&str>|
     -> Result<(), Box<dyn std::error::Error>> {
        let mut tool = Command::new("/usr/bin/python");
        tool.arg(repo.join("tests/support/native_fixture_a11y.py"))
            .arg(&pid)
            .arg(&executable)
            .arg(&data)
            .args([command, role, name]);
        if let Some(value) = value {
            tool.arg(value);
        }
        let output = tool.output()?;
        if !output.status.success() {
            eprintln!(
                "Native observer: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(format!("Owned native action failed: {command} {role} {name}").into());
        }
        Ok(())
    };
    action("wait", "button", "native-folder", None)?;
    action("click", "button", "native-folder", None)?;
    action("key", "", "context", None)?;
    action("click", "menu item", "Compress…", None)?;
    action("set", "entry", "Archive name", Some("nativeui"))?;
    action("click", "check box", "Protect with password", None)?;
    action(
        "set",
        "entry|password text",
        "Password",
        Some("testonlypassword"),
    )?;
    action(
        "set",
        "entry|password text",
        "Confirm password",
        Some("testonlypassword"),
    )?;
    action("click", "button", "Create", None)?;
    action("wait", "button", "nativeui", None)?;
    let archive = scope.child.child_path("nativeui.zip")?;
    let downloaded = scope.local.join("verify-native-ui.zip");
    scope.provider.download_file(&archive, &downloaded, None)?;
    let password_error = crate::commands::decompress::extract_staged(
        None,
        downloaded.to_string_lossy().into_owned(),
        None,
        None,
        None,
    )
    .err()
    .expect("Native password checkbox must produce an encrypted ZIP");
    assert_eq!(password_error.code, "archive_password_required");
    action("click", "button", "nativeui", None)?;
    action("key", "", "context", None)?;
    action("click", "menu item", "Extract", None)?;
    action(
        "set",
        "entry|password text",
        "Password",
        Some("testonlypassword"),
    )?;
    action("click", "button", "Extract", None)?;
    let started = Instant::now();
    let target = loop {
        let outputs = scope
            .provider
            .list_dir(&scope.child)?
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    crate::commands::cloud::types::CloudEntryKind::Dir
                ) && entry.name != "native-folder"
            })
            .collect::<Vec<_>>();
        assert!(
            outputs.len() <= 1,
            "Unexpected extra test output directories"
        );
        if let Some(entry) = outputs.first() {
            action("wait", "button", &entry.name, None)?;
            break scope.child.child_path(&entry.name)?;
        }
        if started.elapsed() > Duration::from_secs(120) {
            return Err("Native extracted tree did not upload".into());
        }
        thread::sleep(Duration::from_millis(250));
    };
    let returned = scope.local.join("returned-native-tree");
    execute_mixed_entry_to_blocking_with_cli(
        scope.provider.cli(),
        MixedTransferOp::Copy,
        MixedTransferPair {
            src: LocalOrCloudArg::Cloud(target),
            dst: LocalOrCloudArg::Local(returned.clone()),
            cloud_remote_for_error_mapping: Some(scope.child.remote().into()),
        },
        MixedTransferWriteOptions::default(),
        None,
        None,
    )?;
    assert_eq!(
        fs::read(returned.join("content.txt"))?,
        b"native UI cloud acceptance\n"
    );
    assert!(returned.join("empty/nested").is_dir());
    let original = scope.local.join("original-native-content.txt");
    scope
        .provider
        .download_file(&source.child_path("content.txt")?, &original, None)?;
    assert_eq!(fs::read(original)?, b"native UI cloud acceptance\n");
    let archive_again = scope.local.join("archive-again.zip");
    scope
        .provider
        .download_file(&archive, &archive_again, None)?;
    assert_eq!(fs::read(downloaded)?, fs::read(archive_again)?);
    eprintln!("PASS: native context-menu password ZIP creation and password-modal extraction through real IPC/staging/OneDrive; original folder/archive retained; byte and empty-directory verification");
    // Use the normal close/exit hooks so our private rclone daemon stops too.
    action("click", "button", "Close window", None)?;
    let closing = Instant::now();
    while candidate.child.try_wait()?.is_none() && closing.elapsed() < Duration::from_secs(10) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        candidate.child.try_wait()?.is_some(),
        "Our completed candidate did not close normally"
    );
    drop(candidate);
    scope.finish()?;
    Ok(())
}

use super::*;
use crate::commands::cloud::{acceptance_tests::OneDriveFixture, provider::CloudProvider};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};

struct Proxy(Child);
impl Drop for Proxy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

fn active_bytes(log: &Path) -> Option<u64> {
    fs::read_to_string(log)
        .ok()?
        .lines()
        .rev()
        .find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            value
                .get("stats")?
                .get("bytes")?
                .as_u64()
                .filter(|bytes| *bytes > 0)
        })
}

#[test]
fn active_fault_detection_requires_real_positive_stats_not_a_spawn_marker() {
    let base = FakeRcloneSandbox::new();
    let log = base.root.join("stats.jsonl");
    fs::write(&log, "spawned\n{\"stats\":{\"bytes\":0}}\n").unwrap();
    assert_eq!(active_bytes(&log), None);
    fs::write(&log, "{\"stats\":{\"bytes\":65536}}\npartial json").unwrap();
    assert_eq!(active_bytes(&log), Some(65536));
}

#[test]
#[ignore = "Active cancellation/network fault only inside an explicitly approved OneDrive fixture"]
fn real_onedrive_active_fault_acceptance() -> Result<(), Box<dyn std::error::Error>> {
    let scope = OneDriveFixture::new()?;
    let source = scope.local.join("recoverable-source.bin");
    let contents = (0..8 * 1024 * 1024_u32)
        .map(|index| (index.wrapping_mul(31) ^ (index >> 8)) as u8)
        .collect::<Vec<_>>();
    fs::write(&source, &contents)?;
    let real_rclone = which::which("rclone")?;
    for network_fault in [false, true] {
        let label = if network_fault { "network" } else { "cancel" };
        let stats = scope.local.join(format!("{label}-stats.jsonl"));
        let drop_marker = scope.local.join("drop-tunnels");
        let mut proxy = None;
        let proxy_env = if network_fault {
            let port_file = scope.local.join("proxy-port.json");
            proxy = Some(Proxy(
                Command::new(which::which("node")?)
                    .arg(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/support/cloud-fault-proxy.mjs"),
                    )
                    .arg(&port_file)
                    .arg(&drop_marker)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?,
            ));
            let start = Instant::now();
            while !port_file.exists() && start.elapsed() < Duration::from_secs(10) {
                thread::sleep(Duration::from_millis(25));
            }
            let value: serde_json::Value = serde_json::from_slice(&fs::read(port_file)?)?;
            let port = value["port"]
                .as_u64()
                .filter(|port| *port > 0 && *port <= 65535)
                .ok_or("Invalid private proxy port")?;
            format!("export HTTPS_PROXY=http://127.0.0.1:{port} https_proxy=http://127.0.0.1:{port} NO_PROXY='' no_proxy=''\n")
        } else {
            String::new()
        };
        let wrapper = scope.local.join(format!("rclone-{label}.sh"));
        // Exec keeps rclone as the registered child PID. tee preserves stderr
        // for production error mapping while storing fixture-only stats.
        fs::write(&wrapper, format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nfor arg in \"$@\"; do\n if [[ \"$arg\" == copyto ]]; then\n{proxy_env}exec {} \"$@\" --bwlimit 256k --stats 200ms --stats-log-level NOTICE --use-json-log --retries 1 --low-level-retries 1 --timeout 3s --contimeout 3s 2> >(tee {} >&2)\n fi\ndone\nexec {} \"$@\"\n",
            quoted(&real_rclone), quoted(&stats), quoted(&real_rclone)
        ))?;
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700))?;
        let cli = RcloneCli::new(wrapper.as_os_str());
        let target = scope
            .child
            .child_path(&format!("{label}-interrupted.bin"))?;
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
        let observed = loop {
            if let Some(bytes) = active_bytes(&stats).filter(|bytes| *bytes < contents.len() as u64)
            {
                break Some(bytes);
            }
            if worker.is_finished() || started.elapsed() > Duration::from_secs(60) {
                break None;
            }
            thread::sleep(Duration::from_millis(25));
        };
        let interrupted = Instant::now();
        if network_fault && observed.is_some() {
            if let Err(error) = fs::write(&drop_marker, b"drop owned tunnels") {
                cancel.store(true, Ordering::SeqCst);
                let _ = worker.join();
                return Err(error.into());
            }
        } else {
            cancel.store(true, Ordering::SeqCst);
        }
        while !worker.is_finished() && interrupted.elapsed() < Duration::from_secs(30) {
            thread::sleep(Duration::from_millis(25));
        }
        if !worker.is_finished() {
            cancel.store(true, Ordering::SeqCst);
        }
        let result = worker
            .join()
            .map_err(|_| "Owned transfer worker panicked")?;
        let return_ms = interrupted.elapsed().as_secs_f64() * 1000.0;
        drop(proxy);
        let bytes = observed
            .ok_or("No positive in-progress stats observed; active-fault acceptance not proven")?;
        let error = result.expect_err("An interrupted transfer must not report success");
        assert!(
            !error.message().contains("https://") && !error.message().contains("tempauth="),
            "Signed provider URLs must not reach transfer error feedback"
        );
        if network_fault {
            assert!(
                matches!(error.code_str(), "network_error" | "timeout"),
                "Expected typed transport failure, got {}",
                error.code_str()
            );
        } else {
            assert_eq!(error.code_str(), "cancelled");
        }
        assert_eq!(
            fs::read(&source)?,
            contents,
            "Source/working bytes must survive failure"
        );
        // Destination completion is unknown after interruption; never infer
        // success or erase it independently. Owner-checked scope cleanup follows.
        let remote_state = scope.provider.stat_path(&target)?;
        eprintln!("PASS: {label} fault after {bytes} rclone-accounted bytes, result {}, return latency {:.1} ms, remote exists {}, source retained",
            error.code_str(), return_ms, remote_state.is_some());
    }
    scope.finish()?;
    Ok(())
}

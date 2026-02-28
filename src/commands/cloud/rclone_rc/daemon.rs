use super::{
    run_rc_command_via_socket, RcloneCliError, RcloneRcMethod, RcloneSubcommand,
    RCLONE_RC_ENABLE_ENV, RCLONE_RC_NOOP_TIMEOUT, RCLONE_RC_READ_ENABLE_ENV,
    RCLONE_RC_STARTUP_POLL_SLICE, RCLONE_RC_STARTUP_TIMEOUT, RCLONE_RC_STATE_DIR_NAME,
    RCLONE_RC_WRITE_ENABLE_ENV,
};
use serde_json::json;
use std::{
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tracing::debug;
use wait_timeout::ChildExt;

#[derive(Debug)]
pub(super) struct RcloneRcDaemon {
    pub(super) socket_path: PathBuf,
    pub(super) binary: OsString,
    pub(super) child: Child,
}

#[derive(Debug, Default)]
pub(super) struct RcloneRcState {
    pub(super) daemon: Option<RcloneRcDaemon>,
    pub(super) startup_blocked_until: Option<Instant>,
    pub(super) startup_blocked_binary: Option<OsString>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RcloneRcHealth {
    pub enabled: bool,
    pub read_enabled: bool,
    pub write_enabled: bool,
    pub daemon_running: bool,
    pub socket_path: Option<String>,
    pub socket_exists: bool,
}

pub(super) fn should_recycle_daemon_after_error(
    _method: super::RcloneRcMethod,
    error: &RcloneCliError,
) -> bool {
    match error {
        RcloneCliError::Timeout { .. } => true,
        RcloneCliError::Io(io) => matches!(
            io.kind(),
            io::ErrorKind::TimedOut
                | io::ErrorKind::WouldBlock
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionRefused
                | io::ErrorKind::Interrupted
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::NotConnected
                | io::ErrorKind::NotFound
        ),
        _ => false,
    }
}

pub fn begin_shutdown_and_kill_daemon() -> io::Result<()> {
    let mut state = match rclone_rc_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(mut daemon) = state.daemon.take() {
        let socket_display = daemon.socket_path.display().to_string();
        kill_daemon(&mut daemon)?;
        debug!(socket = %socket_display, "stopped rclone rcd daemon");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn reset_state_for_tests() {
    let mut state = match rclone_rc_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(mut daemon) = state.daemon.take() {
        let _ = kill_daemon(&mut daemon);
    }
    state.startup_blocked_until = None;
    state.startup_blocked_binary = None;
}

pub fn health_snapshot() -> RcloneRcHealth {
    let read_enabled = rc_read_enabled();
    let write_enabled = rc_write_enabled();
    let enabled = read_enabled || write_enabled;
    let mut daemon_running = false;
    let mut socket_path = None;
    let mut socket_exists = false;

    if let Ok(mut state) = rclone_rc_state().lock() {
        if let Some(daemon) = state.daemon.as_mut() {
            socket_path = Some(daemon.socket_path.display().to_string());
            socket_exists = daemon.socket_path.exists();
            daemon_running = daemon
                .child
                .try_wait()
                .ok()
                .and_then(|status| status)
                .is_none();
        }
    }

    RcloneRcHealth {
        enabled,
        read_enabled,
        write_enabled,
        daemon_running,
        socket_path,
        socket_exists,
    }
}

pub(super) fn parse_rc_toggle_value(value: &str) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_rc_toggle_env(var_name: &str) -> Option<bool> {
    let value = env::var(var_name).ok()?;
    parse_rc_toggle_value(&value)
}

pub(super) fn rc_read_enabled() -> bool {
    static READ_ENABLED: OnceLock<bool> = OnceLock::new();
    *READ_ENABLED.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return false;
        }
        if let Some(all_enabled) = parse_rc_toggle_env(RCLONE_RC_ENABLE_ENV) {
            return all_enabled;
        }
        parse_rc_toggle_env(RCLONE_RC_READ_ENABLE_ENV).unwrap_or(true)
    })
}

pub(super) fn rc_write_enabled() -> bool {
    static WRITE_ENABLED: OnceLock<bool> = OnceLock::new();
    *WRITE_ENABLED.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return false;
        }
        if let Some(all_enabled) = parse_rc_toggle_env(RCLONE_RC_ENABLE_ENV) {
            return all_enabled;
        }
        parse_rc_toggle_env(RCLONE_RC_WRITE_ENABLE_ENV).unwrap_or(true)
    })
}

pub(super) fn rclone_rc_state() -> &'static Mutex<RcloneRcState> {
    static STATE: OnceLock<Mutex<RcloneRcState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RcloneRcState::default()))
}

pub(super) fn spawn_daemon(binary: &OsString) -> Result<RcloneRcDaemon, RcloneCliError> {
    let startup_started = Instant::now();
    let state_dir = rc_state_dir_path()?;
    prepare_state_dir(&state_dir).map_err(RcloneCliError::Io)?;
    let socket_path = state_dir.join(format!("rcd-{}.sock", std::process::id()));
    cleanup_stale_socket(&socket_path).map_err(RcloneCliError::Io)?;

    let mut child = Command::new(binary)
        .arg("rcd")
        .arg("--rc-no-auth")
        .arg("--rc-addr")
        .arg(format!("unix://{}", socket_path.display()))
        .arg("--rc-server-read-timeout")
        .arg("5m")
        .arg("--rc-server-write-timeout")
        .arg("5m")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(RcloneCliError::Io)?;

    let ready_deadline = Instant::now() + RCLONE_RC_STARTUP_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().map_err(RcloneCliError::Io)? {
            return Err(RcloneCliError::NonZero {
                status,
                stdout: String::new(),
                stderr: "rclone rcd exited during startup".to_string(),
            });
        }
        if socket_path.exists() {
            harden_and_validate_socket_path(&socket_path).map_err(RcloneCliError::Io)?;
            match run_rc_command_via_socket(
                &socket_path,
                RcloneRcMethod::CoreNoop,
                json!({}),
                RCLONE_RC_NOOP_TIMEOUT,
            ) {
                Ok(_) => {
                    debug!(
                        socket = %socket_path.display(),
                        startup_ms = startup_started.elapsed().as_millis() as u64,
                        "rclone rcd daemon is ready"
                    );
                    break;
                }
                Err(error) => {
                    debug!(error = %error, "rclone rcd startup probe not ready yet");
                }
            }
        }
        if Instant::now() >= ready_deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RcloneCliError::Timeout {
                subcommand: RcloneSubcommand::Rc,
                timeout: RCLONE_RC_STARTUP_TIMEOUT,
                stdout: String::new(),
                stderr: "timed out waiting for rclone rcd startup".to_string(),
            });
        }
        std::thread::sleep(RCLONE_RC_STARTUP_POLL_SLICE);
    }

    Ok(RcloneRcDaemon {
        socket_path,
        binary: binary.clone(),
        child,
    })
}

pub(super) fn daemon_is_running(daemon: &mut RcloneRcDaemon) -> Result<bool, RcloneCliError> {
    match daemon.child.try_wait().map_err(RcloneCliError::Io)? {
        Some(status) => {
            debug!(status = %status, "rclone rcd daemon has exited");
            Ok(false)
        }
        None => Ok(true),
    }
}

pub(super) fn kill_daemon(daemon: &mut RcloneRcDaemon) -> io::Result<()> {
    let _ = daemon.child.kill();
    let _ = daemon.child.wait_timeout(Duration::from_secs(1));
    let _ = daemon.child.wait();
    if daemon.socket_path.exists() {
        let _ = fs::remove_file(&daemon.socket_path);
    }
    Ok(())
}

fn rc_state_dir_path() -> Result<PathBuf, RcloneCliError> {
    if let Some(xdg_runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(xdg_runtime_dir).join(RCLONE_RC_STATE_DIR_NAME));
    }
    Err(RcloneCliError::Io(io::Error::new(
        io::ErrorKind::NotFound,
        "XDG_RUNTIME_DIR is not set; cannot initialize secure rclone rc socket path",
    )))
}

pub(super) fn prepare_state_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn harden_and_validate_socket_path(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};

        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "rc socket path is not a unix socket",
            ));
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        let mode = fs::symlink_metadata(path)?.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("rc socket path has insecure permissions: {mode:o}"),
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub(super) fn cleanup_stale_socket(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_dir() {
                return Err(io::Error::other(
                    "rc socket path points to a directory; refusing to remove",
                ));
            }
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cleanup_stale_socket, harden_and_validate_socket_path, kill_daemon, parse_rc_toggle_value,
        prepare_state_dir, should_recycle_daemon_after_error, RcloneRcDaemon,
    };
    use crate::commands::cloud::rclone_cli::{RcloneCliError, RcloneSubcommand};
    use crate::commands::cloud::rclone_rc::{
        RcloneRcMethod, RCLONE_RC_READ_TIMEOUT, RCLONE_RC_WRITE_TIMEOUT,
    };
    use std::io;
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::fs::symlink,
        os::unix::net::UnixListener,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    fn unique_test_dir(label: &str) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let unique = format!(
            "browsey-rclone-rc-test-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
                + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).expect("create temp test dir");
        root
    }

    #[test]
    fn parses_rc_toggle_values() {
        assert_eq!(parse_rc_toggle_value("1"), Some(true));
        assert_eq!(parse_rc_toggle_value("true"), Some(true));
        assert_eq!(parse_rc_toggle_value("ON"), Some(true));
        assert_eq!(parse_rc_toggle_value("0"), Some(false));
        assert_eq!(parse_rc_toggle_value("false"), Some(false));
        assert_eq!(parse_rc_toggle_value("Off"), Some(false));
        assert_eq!(parse_rc_toggle_value("maybe"), None);
    }

    #[test]
    fn recycle_policy_matches_transport_failures() {
        assert!(should_recycle_daemon_after_error(
            RcloneRcMethod::OperationsList,
            &RcloneCliError::Timeout {
                subcommand: RcloneSubcommand::Rc,
                timeout: RCLONE_RC_READ_TIMEOUT,
                stdout: String::new(),
                stderr: String::new(),
            }
        ));
        assert!(should_recycle_daemon_after_error(
            RcloneRcMethod::OperationsList,
            &RcloneCliError::Io(io::Error::new(io::ErrorKind::BrokenPipe, "socket closed"))
        ));
        assert!(should_recycle_daemon_after_error(
            RcloneRcMethod::OperationsDeleteFile,
            &RcloneCliError::Io(io::Error::new(io::ErrorKind::TimedOut, "write timed out"))
        ));
        assert!(should_recycle_daemon_after_error(
            RcloneRcMethod::OperationsCopyFile,
            &RcloneCliError::Io(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "connect refused",
            ))
        ));
        assert!(!should_recycle_daemon_after_error(
            RcloneRcMethod::OperationsMkdir,
            &RcloneCliError::Shutdown {
                subcommand: RcloneSubcommand::Rc,
            }
        ));
        let _ = RCLONE_RC_WRITE_TIMEOUT;
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_kill_daemon_terminates_process_and_cleans_socket_path() {
        let root = unique_test_dir("kill-daemon");
        let socket_path = root.join("rc.sock");
        fs::write(&socket_path, b"socket").expect("write placeholder socket file");

        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .spawn()
            .expect("spawn sleep");

        let mut daemon = RcloneRcDaemon {
            socket_path: socket_path.clone(),
            binary: "rclone".into(),
            child,
        };

        kill_daemon(&mut daemon).expect("kill daemon");
        let status = daemon.child.try_wait().expect("wait status");
        assert!(status.is_some(), "daemon child should be terminated");
        assert!(!socket_path.exists(), "socket path should be removed");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_prepare_state_dir_enforces_user_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_test_dir("prepare-state-dir");
        let state_dir = root.join("state");
        prepare_state_dir(&state_dir).expect("prepare state dir");
        let mode = fs::metadata(&state_dir)
            .expect("state dir metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_socket_hardening_enforces_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_test_dir("socket-hardening");
        let socket_path = root.join("rc.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind unix socket");
        harden_and_validate_socket_path(&socket_path).expect("harden socket path");
        let mode = fs::symlink_metadata(&socket_path)
            .expect("socket metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode & 0o077, 0);
        drop(listener);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_socket_hardening_rejects_non_socket_paths() {
        let root = unique_test_dir("socket-hardening-rejects-file");
        let file_path = root.join("not-a-socket");
        fs::write(&file_path, b"nope").expect("write regular file");
        let err = harden_and_validate_socket_path(&file_path).expect_err("should reject file");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_cleanup_stale_socket_removes_symlink_and_regular_file() {
        let root = unique_test_dir("cleanup-stale-socket");
        let stale_socket = root.join("rc.sock");
        let target = root.join("target.sock");
        fs::write(&target, b"target").expect("write target file");
        symlink(&target, &stale_socket).expect("create symlink");
        cleanup_stale_socket(&stale_socket).expect("cleanup stale socket");
        assert!(!stale_socket.exists(), "symlink should be removed");

        fs::write(&stale_socket, b"regular").expect("write regular file");
        cleanup_stale_socket(&stale_socket).expect("cleanup stale regular file");
        assert!(!stale_socket.exists(), "regular file should be removed");
        let _ = fs::remove_dir_all(root);
    }
}

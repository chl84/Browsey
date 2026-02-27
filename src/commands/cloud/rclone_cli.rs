use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};
use wait_timeout::ChildExt;

const RCLONE_DEFAULT_GLOBAL_ARGS: &[&str] =
    &["--retries", "2", "--low-level-retries", "2", "--stats", "0"];
const RCLONE_FAILURE_OUTPUT_MAX_CHARS: usize = 16 * 1024;
const RCLONE_SHUTDOWN_POLL_SLICE_MS: u64 = 100;

type SharedChild = Arc<Mutex<Option<Child>>>;

static RCLONE_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static RCLONE_RUNNING_CHILDREN: OnceLock<Mutex<HashMap<u64, SharedChild>>> = OnceLock::new();
static RCLONE_CHILD_KEY_SEQ: AtomicU64 = AtomicU64::new(1);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcloneSubcommand {
    Version,
    ListRemotes,
    ConfigDump,
    Rc,
    LsJson,
    Mkdir,
    DeleteFile,
    Purge,
    Rmdir,
    MoveTo,
    CopyTo,
}

impl RcloneSubcommand {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::ListRemotes => "listremotes",
            Self::ConfigDump => "config",
            Self::Rc => "rc",
            Self::LsJson => "lsjson",
            Self::Mkdir => "mkdir",
            Self::DeleteFile => "deletefile",
            Self::Purge => "purge",
            Self::Rmdir => "rmdir",
            Self::MoveTo => "moveto",
            Self::CopyTo => "copyto",
        }
    }

    pub fn default_timeout(self) -> Duration {
        match self {
            Self::Version | Self::ListRemotes | Self::ConfigDump => Duration::from_secs(8),
            Self::Rc => Duration::from_secs(45),
            // OneDrive metadata/listing calls can be bursty and occasionally exceed 20s.
            Self::LsJson => Duration::from_secs(60),
            Self::Mkdir => Duration::from_secs(45),
            Self::DeleteFile | Self::Rmdir => Duration::from_secs(120),
            Self::Purge => Duration::from_secs(300),
            Self::MoveTo | Self::CopyTo => Duration::from_secs(300),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcloneCommandSpec {
    subcommand: RcloneSubcommand,
    args: Vec<OsString>,
}

impl RcloneCommandSpec {
    #[allow(dead_code)]
    pub fn new(subcommand: RcloneSubcommand) -> Self {
        Self {
            subcommand,
            args: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[allow(dead_code)]
    pub fn argv(&self) -> Vec<OsString> {
        let mut argv = Vec::with_capacity(2 + self.args.len());
        argv.push(OsString::from(self.subcommand.as_str()));
        if self.subcommand == RcloneSubcommand::ConfigDump {
            argv.push(OsString::from("dump"));
        }
        argv.extend(self.args.iter().cloned());
        argv
    }

    #[allow(dead_code)]
    pub fn into_command(self, binary: &OsStr) -> Command {
        let mut command = Command::new(binary);
        command.arg(self.subcommand.as_str());
        if self.subcommand == RcloneSubcommand::ConfigDump {
            command.arg("dump");
        }
        for arg in self.args {
            command.arg(arg);
        }
        command
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcloneTextOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub enum RcloneCliError {
    Io(std::io::Error),
    Shutdown {
        subcommand: RcloneSubcommand,
    },
    Timeout {
        subcommand: RcloneSubcommand,
        timeout: Duration,
        stdout: String,
        stderr: String,
    },
    NonZero {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
}

impl std::fmt::Display for RcloneCliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Shutdown { subcommand } => {
                write!(
                    f,
                    "rclone {} cancelled because application is shutting down",
                    subcommand.as_str()
                )
            }
            Self::Timeout {
                subcommand,
                timeout,
                stdout,
                stderr,
            } => {
                let mut parts = Vec::new();
                if !stdout.trim().is_empty() {
                    parts.push(format!("stdout: {}", scrub_log_text(stdout)));
                }
                if !stderr.trim().is_empty() {
                    parts.push(format!("stderr: {}", scrub_log_text(stderr)));
                }
                if parts.is_empty() {
                    write!(
                        f,
                        "rclone {} timed out after {}s",
                        subcommand.as_str(),
                        timeout.as_secs()
                    )
                } else {
                    write!(
                        f,
                        "rclone {} timed out after {}s ({})",
                        subcommand.as_str(),
                        timeout.as_secs(),
                        parts.join(" | ")
                    )
                }
            }
            Self::NonZero {
                status,
                stdout,
                stderr,
            } => {
                let mut parts = Vec::new();
                if !stdout.trim().is_empty() {
                    parts.push(format!("stdout: {}", scrub_log_text(stdout)));
                }
                if !stderr.trim().is_empty() {
                    parts.push(format!("stderr: {}", scrub_log_text(stderr)));
                }
                if parts.is_empty() {
                    write!(f, "rclone failed with status {status}")
                } else {
                    write!(
                        f,
                        "rclone failed with status {status} ({})",
                        parts.join(" | ")
                    )
                }
            }
        }
    }
}

impl std::error::Error for RcloneCliError {}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RcloneCli {
    binary: OsString,
}

impl Default for RcloneCli {
    fn default() -> Self {
        Self {
            binary: OsString::from("rclone"),
        }
    }
}

impl RcloneCli {
    #[allow(dead_code)]
    pub fn new(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    #[allow(dead_code)]
    pub fn binary(&self) -> &OsStr {
        &self.binary
    }

    #[allow(dead_code)]
    pub fn command(&self, spec: RcloneCommandSpec) -> Command {
        let mut command = Command::new(&self.binary);
        for arg in RCLONE_DEFAULT_GLOBAL_ARGS {
            command.arg(arg);
        }
        for arg in spec.argv() {
            command.arg(arg);
        }
        command
    }

    pub fn run_capture_text(
        &self,
        spec: RcloneCommandSpec,
    ) -> Result<RcloneTextOutput, RcloneCliError> {
        let subcommand = spec.subcommand;
        let lsjson_stat = subcommand == RcloneSubcommand::LsJson
            && spec.args.iter().any(|arg| arg == &OsString::from("--stat"));
        if RCLONE_SHUTTING_DOWN.load(Ordering::SeqCst) {
            return Err(RcloneCliError::Shutdown { subcommand });
        }
        let timeout = subcommand.default_timeout();
        let started = Instant::now();
        debug!(command = subcommand.as_str(), "running rclone command");
        let mut command = self.command(spec);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let child = Arc::new(Mutex::new(Some(
            command.spawn().map_err(RcloneCliError::Io)?,
        )));
        let _registration = RunningChildRegistration::register(child.clone());
        let output = wait_for_child_output_or_cancel(&child, subcommand, timeout, started)?;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            if subcommand == RcloneSubcommand::LsJson {
                info!(
                    command = subcommand.as_str(),
                    stat = lsjson_stat,
                    elapsed_ms,
                    "rclone command succeeded"
                );
            } else {
                debug!(
                    command = subcommand.as_str(),
                    elapsed_ms, "rclone command succeeded"
                );
            }
            Ok(RcloneTextOutput { stdout, stderr })
        } else {
            let stdout = truncate_failure_output(stdout);
            let stderr = truncate_failure_output(stderr);
            let stderr_preview = scrub_log_text(&stderr);
            let stdout_preview = scrub_log_text(&stdout);
            warn!(
                command = subcommand.as_str(),
                elapsed_ms,
                status = %output.status,
                stderr = %stderr_preview,
                stdout = %stdout_preview,
                "rclone command failed"
            );
            Err(RcloneCliError::NonZero {
                status: output.status,
                stdout,
                stderr,
            })
        }
    }
}

pub(crate) fn begin_shutdown_and_kill_children() -> usize {
    RCLONE_SHUTTING_DOWN.store(true, Ordering::SeqCst);
    let children = match rclone_child_registry().lock() {
        Ok(registry) => registry.values().cloned().collect::<Vec<_>>(),
        Err(_) => return 0,
    };
    let mut killed = 0usize;
    for child in children {
        if child_kill(&child).is_ok() {
            killed += 1;
        }
    }
    if killed > 0 {
        debug!(
            killed,
            "requested shutdown of running rclone child processes"
        );
    }
    killed
}

fn wait_for_child_output_or_cancel(
    child: &SharedChild,
    subcommand: RcloneSubcommand,
    timeout: Duration,
    started: Instant,
) -> Result<Output, RcloneCliError> {
    let poll = Duration::from_millis(RCLONE_SHUTDOWN_POLL_SLICE_MS);
    loop {
        if RCLONE_SHUTTING_DOWN.load(Ordering::SeqCst) {
            let _ = child_kill(child);
            let _ = child_wait_with_output(child);
            return Err(RcloneCliError::Shutdown { subcommand });
        }

        let elapsed = started.elapsed();
        if elapsed >= timeout {
            let _ = child_kill(child);
            let output = child_wait_with_output(child).map_err(RcloneCliError::Io)?;
            let stdout =
                truncate_failure_output(String::from_utf8_lossy(&output.stdout).into_owned());
            let stderr =
                truncate_failure_output(String::from_utf8_lossy(&output.stderr).into_owned());
            let elapsed_ms = elapsed.as_millis() as u64;
            warn!(
                command = subcommand.as_str(),
                elapsed_ms,
                timeout_ms = timeout.as_millis() as u64,
                stderr = %scrub_log_text(&stderr),
                stdout = %scrub_log_text(&stdout),
                "rclone command timed out"
            );
            return Err(RcloneCliError::Timeout {
                subcommand,
                timeout,
                stdout,
                stderr,
            });
        }

        let remaining = timeout.saturating_sub(elapsed);
        let slice = remaining.min(poll);
        match child_wait_timeout(child, slice).map_err(RcloneCliError::Io)? {
            Some(_) => {
                return child_wait_with_output(child).map_err(RcloneCliError::Io);
            }
            None => continue,
        }
    }
}

fn rclone_child_registry() -> &'static Mutex<HashMap<u64, SharedChild>> {
    RCLONE_RUNNING_CHILDREN.get_or_init(|| Mutex::new(HashMap::new()))
}

struct RunningChildRegistration {
    key: u64,
}

impl RunningChildRegistration {
    fn register(child: SharedChild) -> Self {
        let key = RCLONE_CHILD_KEY_SEQ.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut registry) = rclone_child_registry().lock() {
            registry.insert(key, child);
        }
        Self { key }
    }
}

impl Drop for RunningChildRegistration {
    fn drop(&mut self) {
        if let Ok(mut registry) = rclone_child_registry().lock() {
            registry.remove(&self.key);
        }
    }
}

fn child_wait_timeout(
    child: &SharedChild,
    timeout: Duration,
) -> std::io::Result<Option<ExitStatus>> {
    let mut guard = child
        .lock()
        .map_err(|_| std::io::Error::other("failed to lock rclone child process"))?;
    let child = guard
        .as_mut()
        .ok_or_else(|| std::io::Error::other("rclone child process already consumed"))?;
    child.wait_timeout(timeout)
}

fn child_kill(child: &SharedChild) -> std::io::Result<()> {
    let mut guard = child
        .lock()
        .map_err(|_| std::io::Error::other("failed to lock rclone child process"))?;
    match guard.as_mut() {
        Some(child) => child.kill(),
        None => Ok(()),
    }
}

fn child_wait_with_output(child: &SharedChild) -> std::io::Result<Output> {
    let mut guard = child
        .lock()
        .map_err(|_| std::io::Error::other("failed to lock rclone child process"))?;
    let child = guard
        .take()
        .ok_or_else(|| std::io::Error::other("rclone child process already consumed"))?;
    drop(guard);
    child.wait_with_output()
}

fn scrub_log_text(raw: &str) -> String {
    const MAX_CHARS: usize = 320;
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (idx, line) in raw.lines().enumerate() {
        if idx > 0 {
            out.push_str(" | ");
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("token")
            || lower.contains("secret")
            || lower.contains("password")
            || lower.contains("authorization")
        {
            out.push_str("[redacted]");
        } else {
            out.push_str(line.trim());
        }
        if out.chars().count() >= MAX_CHARS {
            let mut truncated = out.chars().take(MAX_CHARS).collect::<String>();
            truncated.push('…');
            return truncated;
        }
    }
    out
}

fn truncate_failure_output(raw: String) -> String {
    if raw.chars().count() <= RCLONE_FAILURE_OUTPUT_MAX_CHARS {
        return raw;
    }
    let mut truncated = raw
        .chars()
        .take(RCLONE_FAILURE_OUTPUT_MAX_CHARS)
        .collect::<String>();
    truncated.push_str("… [truncated]");
    truncated
}

#[cfg(test)]
mod tests {
    use super::{
        scrub_log_text, truncate_failure_output, RcloneCli, RcloneCommandSpec, RcloneSubcommand,
    };
    use std::ffi::OsString;

    #[test]
    fn command_spec_builds_expected_argv() {
        let spec = RcloneCommandSpec::new(RcloneSubcommand::LsJson)
            .arg("--files-only")
            .arg("remote:path");
        let argv = spec.argv();
        assert_eq!(
            argv,
            vec![
                OsString::from("lsjson"),
                OsString::from("--files-only"),
                OsString::from("remote:path")
            ]
        );
    }

    #[test]
    fn cli_uses_configured_binary() {
        let cli = RcloneCli::new("/usr/bin/rclone");
        let command = cli.command(RcloneCommandSpec::new(RcloneSubcommand::Version));
        assert_eq!(
            command.get_program(),
            std::ffi::OsStr::new("/usr/bin/rclone")
        );
    }

    #[test]
    fn cli_injects_default_global_flags() {
        let cli = RcloneCli::default();
        let command = cli.command(RcloneCommandSpec::new(RcloneSubcommand::Version));
        let args: Vec<_> = command.get_args().map(|a| a.to_os_string()).collect();
        assert_eq!(
            args,
            vec![
                OsString::from("--retries"),
                OsString::from("2"),
                OsString::from("--low-level-retries"),
                OsString::from("2"),
                OsString::from("--stats"),
                OsString::from("0"),
                OsString::from("version"),
            ]
        );
    }

    #[test]
    fn config_dump_builds_two_word_subcommand() {
        let spec = RcloneCommandSpec::new(RcloneSubcommand::ConfigDump);
        let argv = spec.argv();
        assert_eq!(argv, vec![OsString::from("config"), OsString::from("dump")]);
    }

    #[test]
    fn subcommands_have_reasonable_default_timeouts() {
        assert_eq!(RcloneSubcommand::Version.default_timeout().as_secs(), 8);
        assert_eq!(RcloneSubcommand::LsJson.default_timeout().as_secs(), 60);
        assert_eq!(
            RcloneSubcommand::DeleteFile.default_timeout().as_secs(),
            120
        );
        assert_eq!(RcloneSubcommand::Rmdir.default_timeout().as_secs(), 120);
        assert_eq!(RcloneSubcommand::Purge.default_timeout().as_secs(), 300);
        assert_eq!(RcloneSubcommand::CopyTo.default_timeout().as_secs(), 300);
    }

    #[test]
    fn scrub_log_text_redacts_and_truncates() {
        let scrubbed = scrub_log_text("token=abc123\npermission denied");
        assert!(scrubbed.contains("[redacted]"));
        assert!(scrubbed.contains("permission denied"));

        let long = "x".repeat(500);
        let truncated = scrub_log_text(&long);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn truncates_failure_output_to_bounded_size() {
        let long = "y".repeat(20_000);
        let truncated = truncate_failure_output(long);
        assert!(truncated.ends_with("… [truncated]"));
        assert!(truncated.chars().count() <= 16 * 1024 + "… [truncated]".chars().count());
    }
}

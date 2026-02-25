use std::{
    ffi::{OsStr, OsString},
    process::{Command, ExitStatus},
    time::Instant,
};
use tracing::{debug, warn};

const RCLONE_DEFAULT_GLOBAL_ARGS: &[&str] =
    &["--retries", "2", "--low-level-retries", "2", "--stats", "0"];
const RCLONE_FAILURE_OUTPUT_MAX_CHARS: usize = 16 * 1024;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcloneSubcommand {
    Version,
    ListRemotes,
    ConfigDump,
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
            Self::LsJson => "lsjson",
            Self::Mkdir => "mkdir",
            Self::DeleteFile => "deletefile",
            Self::Purge => "purge",
            Self::Rmdir => "rmdir",
            Self::MoveTo => "moveto",
            Self::CopyTo => "copyto",
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
            Self::NonZero {
                status,
                stdout,
                stderr,
            } => {
                let mut parts = Vec::new();
                if !stdout.trim().is_empty() {
                    parts.push(format!("stdout: {}", stdout.trim()));
                }
                if !stderr.trim().is_empty() {
                    parts.push(format!("stderr: {}", stderr.trim()));
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
        let started = Instant::now();
        debug!(command = subcommand.as_str(), "running rclone command");
        let output = self.command(spec).output().map_err(RcloneCliError::Io)?;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            debug!(
                command = subcommand.as_str(),
                elapsed_ms, "rclone command succeeded"
            );
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

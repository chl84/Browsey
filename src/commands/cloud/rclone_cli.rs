use std::{
    ffi::{OsStr, OsString},
    process::{Command, ExitStatus},
};

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
        spec.into_command(&self.binary)
    }

    pub fn run_capture_text(
        &self,
        spec: RcloneCommandSpec,
    ) -> Result<RcloneTextOutput, RcloneCliError> {
        let output = self.command(spec).output().map_err(RcloneCliError::Io)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            Ok(RcloneTextOutput { stdout, stderr })
        } else {
            Err(RcloneCliError::NonZero {
                status: output.status,
                stdout,
                stderr,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RcloneCli, RcloneCommandSpec, RcloneSubcommand};
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
    fn config_dump_builds_two_word_subcommand() {
        let spec = RcloneCommandSpec::new(RcloneSubcommand::ConfigDump);
        let argv = spec.argv();
        assert_eq!(argv, vec![OsString::from("config"), OsString::from("dump")]);
    }
}

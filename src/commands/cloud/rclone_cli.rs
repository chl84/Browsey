use std::{
    ffi::{OsStr, OsString},
    process::Command,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcloneSubcommand {
    Version,
    ListRemotes,
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
        let mut argv = Vec::with_capacity(1 + self.args.len());
        argv.push(OsString::from(self.subcommand.as_str()));
        argv.extend(self.args.iter().cloned());
        argv
    }

    #[allow(dead_code)]
    pub fn into_command(self, binary: &OsStr) -> Command {
        let mut command = Command::new(binary);
        command.arg(self.subcommand.as_str());
        for arg in self.args {
            command.arg(arg);
        }
        command
    }
}

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
}

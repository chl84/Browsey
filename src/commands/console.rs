use super::open_with::spawn_detached;
use std::path::Path;
use std::process::Command;

use crate::errors::api_error::ApiResult;
use crate::fs_utils::sanitize_path_follow;
use error::{map_api_result, ConsoleError, ConsoleErrorCode, ConsoleResult};

#[cfg(target_os = "linux")]
type Candidate = (&'static str, Vec<String>);

mod error;

#[cfg(target_os = "linux")]
fn linux_terminal_candidates(dir: &str) -> Vec<Candidate> {
    // Strict allowlist: no environment-controlled terminal commands or free-form args.
    vec![
        (
            "ptyxis",
            vec![
                "--new-window".to_string(),
                "--working-directory".to_string(),
                dir.to_string(),
            ],
        ),
        (
            "ptyxis",
            vec![
                "--tab".to_string(),
                "--working-directory".to_string(),
                dir.to_string(),
            ],
        ),
        (
            "gnome-terminal",
            vec!["--working-directory".to_string(), dir.to_string()],
        ),
        ("gnome-terminal", vec![format!("--working-directory={dir}")]),
        ("konsole", vec!["--workdir".to_string(), dir.to_string()]),
        (
            "xfce4-terminal",
            vec!["--working-directory".to_string(), dir.to_string()],
        ),
        (
            "tilix",
            vec!["--working-directory".to_string(), dir.to_string()],
        ),
        (
            "alacritty",
            vec!["--working-directory".to_string(), dir.to_string()],
        ),
        ("kitty", vec!["--directory".to_string(), dir.to_string()]),
        (
            "wezterm",
            vec!["start".to_string(), "--cwd".to_string(), dir.to_string()],
        ),
        ("wezterm", vec!["start".to_string(), format!("--cwd={dir}")]),
        (
            "foot",
            vec!["--working-directory".to_string(), dir.to_string()],
        ),
        (
            "kgx",
            vec!["--working-directory".to_string(), dir.to_string()],
        ),
    ]
}

#[tauri::command]
pub fn open_console(path: String) -> ApiResult<()> {
    map_api_result(open_console_impl(path))
}

fn open_console_impl(path: String) -> ConsoleResult<()> {
    if !Path::new(&path).is_absolute() {
        return Err(ConsoleError::new(
            ConsoleErrorCode::PathNotAbsolute,
            "Path must be absolute",
        ));
    }
    let pb = sanitize_path_follow(&path, true).map_err(ConsoleError::from)?;
    if !pb.is_dir() {
        return Err(ConsoleError::new(
            ConsoleErrorCode::NotDirectory,
            "Can only open console in a directory",
        ));
    }
    launch_terminal(&pb)
}

fn launch_terminal(dir: &Path) -> ConsoleResult<()> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "cmd"]).current_dir(dir);
        spawn_detached(command).map_err(|error| {
            ConsoleError::from_io_error(
                ConsoleErrorCode::LaunchFailed,
                "Failed to launch cmd",
                error,
            )
        })?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.args(["-a", "Terminal"]).arg(dir);
        spawn_detached(command).map_err(|error| {
            ConsoleError::from_io_error(
                ConsoleErrorCode::LaunchFailed,
                "Failed to launch Terminal",
                error,
            )
        })?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // Strict Linux launch strategy: only known terminal binaries/args.
        let dir_arg = dir.to_string_lossy().to_string();
        let mut tried_bins = std::collections::BTreeSet::new();
        let mut launch_error: Option<std::io::Error> = None;
        for (bin, args) in linux_terminal_candidates(&dir_arg) {
            let mut cmdline = Command::new(bin);
            cmdline.current_dir(dir).args(&args);
            match spawn_detached(cmdline) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    tried_bins.insert(bin.to_string());
                    if error.kind() != std::io::ErrorKind::NotFound && launch_error.is_none() {
                        launch_error = Some(error);
                    }
                }
            }
        }
        if let Some(error) = launch_error {
            return Err(ConsoleError::from_io_error(
                ConsoleErrorCode::LaunchFailed,
                "Failed to launch terminal emulator",
                error,
            ));
        }
        return Err(ConsoleError::new(
            ConsoleErrorCode::TerminalUnavailable,
            format!(
                "Could not find a supported terminal emulator to launch (tried: {})",
                tried_bins.into_iter().collect::<Vec<_>>().join(", ")
            ),
        ));
    }

    #[allow(unreachable_code)]
    Err(ConsoleError::new(
        ConsoleErrorCode::UnsupportedPlatform,
        "Unsupported platform for opening console",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::domain::DomainError;
    use std::fs;
    use std::time::{Duration, SystemTime};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-console-test-{label}-{ts}"))
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_terminal_candidates_use_strict_allowlist_and_dir_args() {
        let dir = "/tmp/browsey-console-test";
        let candidates = linux_terminal_candidates(dir);
        assert!(
            !candidates.is_empty(),
            "linux terminal candidate list should not be empty"
        );

        for (bin, args) in &candidates {
            assert!(
                matches!(
                    *bin,
                    "ptyxis"
                        | "gnome-terminal"
                        | "konsole"
                        | "xfce4-terminal"
                        | "tilix"
                        | "alacritty"
                        | "kitty"
                        | "wezterm"
                        | "foot"
                        | "kgx"
                ),
                "unexpected terminal candidate: {bin}"
            );
            assert!(
                args.iter()
                    .any(|arg| arg == dir || arg.ends_with(&format!("={dir}"))),
                "candidate {bin:?} did not propagate working directory: {args:?}"
            );
        }
    }

    #[test]
    fn open_console_rejects_non_directory_paths() {
        let path = temp_path("not-directory");
        fs::write(&path, b"not a directory").expect("create temp file");

        let error = open_console_impl(path.to_string_lossy().to_string())
            .expect_err("opening a console for a file should fail");
        assert_eq!(error.code_str(), "not_directory");
        assert_eq!(error.message(), "Can only open console in a directory");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn open_console_rejects_relative_paths_before_fs_sanitization() {
        let error =
            open_console_impl("relative/path".to_string()).expect_err("relative paths should fail");
        assert_eq!(error.code_str(), "path_not_absolute");
        assert_eq!(error.message(), "Path must be absolute");
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn terminal_children_are_reaped_in_an_isolated_process() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_path("reaper");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let terminal = bin.join("gnome-terminal");
        fs::write(&terminal, "#!/bin/sh\nprintf '%s' \"$$\" > terminal.pid\n").unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "commands::console::tests::terminal_reaper_child",
                "--nocapture",
            ])
            .env("BROWSEY_TERMINAL_REAPER_ROOT", &root)
            .env("PATH", &bin)
            .output()
            .unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn terminal_reaper_child() {
        let Some(root) = std::env::var_os("BROWSEY_TERMINAL_REAPER_ROOT") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        launch_terminal(&root).unwrap();
        let mut pid = None;
        for _ in 0..200 {
            if let Ok(value) = fs::read_to_string(root.join("terminal.pid")) {
                pid = value.trim().parse::<i32>().ok();
                if let Some(pid) = pid {
                    if !Path::new(&format!("/proc/{pid}")).exists() {
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Reap only this owned fixture if the regression leaves a zombie.
        if let Some(pid) = pid {
            unsafe {
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            }
        }
        panic!("terminal child was not reaped");
    }
}

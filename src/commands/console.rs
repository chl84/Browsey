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
    let pb = sanitize_path_follow(&path, true).map_err(ConsoleError::from_external_message)?;
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
        Command::new("cmd")
            .args(["/C", "start", "cmd"])
            .current_dir(dir)
            .spawn()
            .map_err(|error| {
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
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(dir)
            .spawn()
            .map_err(|error| {
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
            match cmdline.spawn() {
                Ok(_) => return Ok(()),
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

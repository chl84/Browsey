use std::path::Path;
use std::process::Command;

use crate::fs_utils::sanitize_path_follow;

#[cfg(target_os = "linux")]
type Candidate = (&'static str, Vec<String>);

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
pub fn open_console(path: String) -> Result<(), String> {
    let pb = sanitize_path_follow(&path, true)?;
    if !pb.is_dir() {
        return Err("Can only open console in a directory".into());
    }
    launch_terminal(&pb)
}

fn launch_terminal(dir: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "cmd"])
            .current_dir(dir)
            .spawn()
            .map_err(|e| format!("Failed to launch cmd: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to launch Terminal: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // Strict Linux launch strategy: only known terminal binaries/args.
        let dir_arg = dir.to_string_lossy().to_string();
        let mut tried_bins = std::collections::BTreeSet::new();
        for (bin, args) in linux_terminal_candidates(&dir_arg) {
            let mut cmdline = Command::new(&bin);
            cmdline.current_dir(dir).args(&args);
            match cmdline.spawn() {
                Ok(_) => return Ok(()),
                Err(_) => {
                    tried_bins.insert(bin.to_string());
                }
            }
        }
        return Err(format!(
            "Could not find a supported terminal emulator to launch (tried: {})",
            tried_bins.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform for opening console".into())
}

use std::path::PathBuf;
use std::process::Command;

use crate::fs_utils::sanitize_path_follow;

#[tauri::command]
pub fn open_console(path: String) -> Result<(), String> {
    let pb = sanitize_path_follow(&path, true)?;
    if !pb.is_dir() {
        return Err("Can only open console in a directory".into());
    }
    launch_terminal(&pb)
}

fn launch_terminal(dir: &PathBuf) -> Result<(), String> {
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
        let script = format!(
            r#"tell application "Terminal"
if not (exists window 1) then
    do script "cd \"{}\""
else
    do script "cd \"{}\"" in window 1
end if
activate
end tell"#,
            dir.display(),
            dir.display()
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to launch Terminal: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let cmd = format!("cd \"{}\"; exec \"$SHELL\"", dir.display());

        // Allow override via env
        let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
        if let Ok(bin) = std::env::var("FILEY_TERMINAL")
            .or_else(|_| std::env::var("TERMINAL"))
            .or_else(|_| std::env::var("COLORTERM"))
        {
            candidates.push((
                bin,
                vec!["-e".into(), "sh".into(), "-c".into(), cmd.clone()],
            ));
        }

        // Common Fedora/GTK/KDE terminals
        let defaults = [
            ("x-terminal-emulator", vec!["-e", "sh", "-c"]),
            ("kgx", vec!["-e", "sh", "-c"]),
            ("gnome-terminal", vec!["--", "sh", "-c"]),
            ("konsole", vec!["-e", "sh", "-c"]),
            ("xfce4-terminal", vec!["-e", "sh", "-c"]),
            ("tilix", vec!["-e", "sh", "-c"]),
            ("alacritty", vec!["-e", "sh", "-c"]),
            ("kitty", vec!["sh", "-c"]),
            ("wezterm", vec!["start", "--", "sh", "-c"]),
            ("foot", vec!["-e", "sh", "-c"]),
            ("rio", vec!["-e", "sh", "-c"]),
            ("ptyxis", vec!["-e", "sh", "-c"]),
        ];
        for (bin, args) in defaults {
            candidates.push((
                bin.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
        }

        let mut tried: Vec<String> = Vec::new();
        for (bin, mut args) in candidates {
            let mut cmdline = Command::new(&bin);
            // Append command payload if not already present
            if !args.is_empty() {
                args.push(cmd.clone());
                cmdline.args(&args);
            } else {
                cmdline.arg(&cmd);
            }
            match cmdline.spawn() {
                Ok(_) => return Ok(()),
                Err(_) => tried.push(bin),
            }
        }
        return Err(format!(
            "Could not find a terminal emulator to launch (tried: {})",
            tried.join(", ")
        ));
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform for opening console".into())
}

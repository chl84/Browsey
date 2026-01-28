use std::process::Command;

use url::Url;

use crate::fs_utils::sanitize_path_follow;

#[derive(serde::Serialize)]
pub struct SystemClipboardContent {
    pub mode: String,
    pub paths: Vec<String>,
}

fn file_uri(path: &str) -> Result<String, String> {
    let cleaned = sanitize_path_follow(path, true)?;
    Url::from_file_path(&cleaned)
        .map_err(|_| "Failed to build file URI".to_string())
        .map(|u| u.to_string())
}

fn run_wl_copy(mime: &str, payload: &str) -> Result<(), String> {
    let status = Command::new("wl-copy")
        .arg("--type")
        .arg(mime)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(payload.as_bytes())?;
            }
            child.wait()
        })
        .map_err(|e| format!("wl-copy failed: {e}"))?;
    if !status.success() {
        return Err(format!("wl-copy exited with status {status}"));
    }
    Ok(())
}

fn run_xclip(mime: &str, payload: &str) -> Result<(), String> {
    let status = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-t")
        .arg(mime)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(payload.as_bytes())?;
            }
            child.wait()
        })
        .map_err(|e| format!("xclip failed: {e}"))?;
    if !status.success() {
        return Err(format!("xclip exited with status {status}"));
    }
    Ok(())
}

fn format_gnome_payload(uris: &[String]) -> String {
    let mut s = String::from("copy\n");
    for uri in uris {
        s.push_str(uri);
        s.push('\n');
    }
    s
}

fn format_uri_list(uris: &[String]) -> String {
    uris.join("\r\n")
}

#[tauri::command]
pub fn copy_paths_to_system_clipboard(paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No paths provided".into());
    }
    let mut uris = Vec::with_capacity(paths.len());
    for p in paths {
        uris.push(file_uri(&p)?);
    }
    let gnome_payload = format_gnome_payload(&uris);
    let uri_list = format_uri_list(&uris);

    if run_wl_copy("x-special/gnome-copied-files", &gnome_payload).is_ok() {
        let _ = run_wl_copy("text/uri-list", &uri_list);
        return Ok(());
    }

    if run_xclip("x-special/gnome-copied-files", &gnome_payload).is_ok() {
        let _ = run_xclip("text/uri-list", &uri_list);
        return Ok(());
    }

    Err("No compatible clipboard tool found (need wl-copy or xclip)".into())
}

fn read_command_output(cmd: &mut Command) -> Result<Option<String>, String> {
    let output = cmd.output().map_err(|e| format!("Clipboard read failed: {e}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(output.stdout).map_err(|e| format!("Clipboard text decode failed: {e}"))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(text))
}

fn read_wl_paste(mime: &str) -> Result<Option<String>, String> {
    read_command_output(Command::new("wl-paste").arg("--type").arg(mime))
}

fn read_xclip(mime: &str) -> Result<Option<String>, String> {
    read_command_output(
        Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .arg("-t")
            .arg(mime)
            .arg("-o"),
    )
}

fn parse_uri_list(payload: &str) -> Vec<String> {
    payload
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            Url::parse(trimmed)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect()
}

fn parse_gnome_payload(payload: &str) -> Option<SystemClipboardContent> {
    let mut lines = payload.lines();
    let mode_line = lines.next()?.trim().to_lowercase();
    let mode = if mode_line == "cut" { "cut" } else { "copy" }.to_string();
    let rest: Vec<&str> = lines.collect();
    let paths = rest
        .iter()
        .filter_map(|l| {
            let trimmed = l.trim();
            if trimmed.is_empty() {
                return None;
            }
            Url::parse(trimmed)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    if paths.is_empty() {
        None
    } else {
        Some(SystemClipboardContent { mode, paths })
    }
}

#[tauri::command]
pub fn system_clipboard_paths() -> Result<SystemClipboardContent, String> {
    // Try Wayland payload first
    if let Some(text) = read_wl_paste("x-special/gnome-copied-files")? {
        if let Some(content) = parse_gnome_payload(&text) {
            return Ok(content);
        }
    }
    if let Some(text) = read_wl_paste("text/uri-list")? {
        let paths = parse_uri_list(&text);
        if !paths.is_empty() {
            return Ok(SystemClipboardContent {
                mode: "copy".into(),
                paths,
            });
        }
    }

    // Fallback to X11
    if let Some(text) = read_xclip("x-special/gnome-copied-files")? {
        if let Some(content) = parse_gnome_payload(&text) {
            return Ok(content);
        }
    }
    if let Some(text) = read_xclip("text/uri-list")? {
        let paths = parse_uri_list(&text);
        if !paths.is_empty() {
            return Ok(SystemClipboardContent {
                mode: "copy".into(),
                paths,
            });
        }
    }

    Err("No file paths found in system clipboard".into())
}

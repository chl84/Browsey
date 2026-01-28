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

#[tauri::command]
pub fn copy_paths_to_system_clipboard(paths: Vec<String>, mode: Option<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No paths provided".into());
    }
    let mut uris = Vec::with_capacity(paths.len());
    for p in paths {
        uris.push(file_uri(&p)?);
    }
    let action = match mode.unwrap_or_else(|| "copy".into()).to_lowercase().as_str() {
        "cut" => "cut",
        _ => "copy",
    };
    let uri_list = {
        let mut lines = Vec::with_capacity(uris.len() + 1);
        if action == "cut" {
            // Hint consumers that this is a move when x-special payload is unavailable.
            lines.push("#cut".to_string());
        }
        lines.extend(uris.iter().cloned());
        lines.join("\r\n")
    };
    let gnome_payload = {
        let mut s = String::from(action);
        s.push('\n');
        for uri in &uris {
            s.push_str(uri);
            s.push('\n');
        }
        s
    };

    // Write both payloads: gnome for cut/copy semantics, uri-list for compatibility.
    let mut wrote = false;
    if run_wl_copy("x-special/gnome-copied-files", &gnome_payload).is_ok() {
        let _ = run_wl_copy("text/uri-list", &uri_list);
        wrote = true;
    } else if run_xclip("x-special/gnome-copied-files", &gnome_payload).is_ok() {
        let _ = run_xclip("text/uri-list", &uri_list);
        wrote = true;
    }

    if wrote {
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

fn parse_uri_list(payload: &str) -> (Vec<String>, String) {
    let mut mode = "copy".to_string();
    let mut paths = Vec::new();
    for line in payload.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            if trimmed.eq_ignore_ascii_case("#cut") {
                mode = "cut".to_string();
            }
            continue;
        }
        if let Ok(url) = Url::parse(trimmed) {
            if let Ok(path) = url.to_file_path() {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    (paths, mode)
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
        let (paths, mode) = parse_uri_list(&text);
        if !paths.is_empty() {
            return Ok(SystemClipboardContent {
                mode,
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
        let (paths, mode) = parse_uri_list(&text);
        if !paths.is_empty() {
            return Ok(SystemClipboardContent {
                mode,
                paths,
            });
        }
    }

    Err("No file paths found in system clipboard".into())
}

fn clear_with_wl_copy() -> Result<(), String> {
    let status = Command::new("wl-copy")
        .arg("--clear")
        .status()
        .map_err(|e| format!("wl-copy --clear failed: {e}"))?;
    if !status.success() {
        return Err(format!("wl-copy --clear exited with status {status}"));
    }
    Ok(())
}

fn clear_with_xclip() -> Result<(), String> {
    let status = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-i")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(|e| format!("xclip clear failed: {e}"))?;
    if !status.success() {
        return Err(format!("xclip clear exited with status {status}"));
    }
    Ok(())
}

#[tauri::command]
pub fn clear_system_clipboard() -> Result<(), String> {
    if clear_with_wl_copy().is_ok() {
        return Ok(());
    }
    if clear_with_xclip().is_ok() {
        return Ok(());
    }
    Err("No compatible clipboard tool found (need wl-copy or xclip)".into())
}

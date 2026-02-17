#[cfg(not(target_os = "windows"))]
use std::{collections::HashSet, fs, path::PathBuf};

#[cfg(not(target_os = "windows"))]
fn is_sftp_uri(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    lower.starts_with("sftp://") || lower.starts_with("ssh://")
}

#[cfg(not(target_os = "windows"))]
fn sftp_host(uri: &str) -> Option<String> {
    let (_, remainder) = uri.split_once("://")?;
    let authority = remainder.split('/').next()?.trim();
    if authority.is_empty() {
        return None;
    }
    let authority = authority.rsplit('@').next().unwrap_or(authority).trim();
    if authority.is_empty() {
        return None;
    }
    if authority.starts_with('[') {
        if let Some(end) = authority.find(']') {
            let host = authority[1..end].trim();
            if !host.is_empty() {
                return Some(host.to_string());
            }
        }
    }
    let host = authority.split(':').next().unwrap_or(authority).trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn sftp_label(uri: &str, display_label: Option<&str>) -> String {
    let explicit = display_label.unwrap_or("").trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    if let Some(host) = sftp_host(uri) {
        return format!("SFTP ({host})");
    }
    "SFTP".to_string()
}

#[cfg(not(target_os = "windows"))]
fn parse_gtk_bookmarks(contents: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(uri) = parts.next() else { continue };
        if !is_sftp_uri(uri) {
            continue;
        }
        let key = uri.trim_end_matches('/').to_ascii_lowercase();
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        let label_raw = parts.collect::<Vec<_>>().join(" ");
        let label = if label_raw.is_empty() {
            sftp_label(uri, None)
        } else {
            sftp_label(uri, Some(&label_raw))
        };
        entries.push((label, uri.to_string()));
    }
    entries
}

#[cfg(not(target_os = "windows"))]
fn bookmark_sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(config) = dirs_next::config_dir() {
        files.push(config.join("gtk-3.0").join("bookmarks"));
        files.push(config.join("gtk-4.0").join("bookmarks"));
    }
    if let Some(home) = dirs_next::home_dir() {
        files.push(home.join(".gtk-bookmarks"));
    }
    files
}

#[cfg(not(target_os = "windows"))]
pub fn list_sftp_mountables() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for source in bookmark_sources() {
        let Ok(contents) = fs::read_to_string(source) else {
            continue;
        };
        for (label, uri) in parse_gtk_bookmarks(&contents) {
            let key = uri.trim_end_matches('/').to_ascii_lowercase();
            if key.is_empty() || !seen.insert(key) {
                continue;
            }
            out.push((label, uri));
        }
    }
    out
}

#[cfg(target_os = "windows")]
pub fn list_sftp_mountables() -> Vec<(String, String)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gtk_bookmarks_extracts_sftp_and_label() {
        let text = r#"
# comment
sftp://alice@example.com/ Work server
file:///tmp Local
ssh://root@box.internal/ Root shell
        "#;
        let parsed = parse_gtk_bookmarks(text);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "Work server");
        assert_eq!(parsed[0].1, "sftp://alice@example.com/");
        assert_eq!(parsed[1].1, "ssh://root@box.internal/");
    }

    #[test]
    fn parse_gtk_bookmarks_dedupes_and_builds_fallback_label() {
        let text = r#"
sftp://alice@example.com
sftp://alice@example.com/
        "#;
        let parsed = parse_gtk_bookmarks(text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "SFTP (example.com)");
    }

    #[test]
    fn sftp_host_handles_user_and_port() {
        assert_eq!(
            sftp_host("sftp://alice@example.com:2222/path").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            sftp_host("sftp://[2001:db8::1]:2222/path").as_deref(),
            Some("2001:db8::1")
        );
    }
}

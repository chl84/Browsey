//! Shared URI classification and GVFS mount-path resolution for network entries.

use crate::commands::fs::MountInfo;
use serde::Serialize;
use std::collections::HashMap;

use super::mounts;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkUriKind {
    NotUri,
    Mountable,
    External,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkUriClassification {
    pub kind: NetworkUriKind,
    pub scheme: Option<String>,
    pub normalized_uri: Option<String>,
}

pub(crate) fn canonical_scheme(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "onedrive" => Some("onedrive"),
        "sftp" | "ssh" | "sshfs" | "fuse.sshfs" => Some("sftp"),
        "smb" | "smb3" | "smbfs" | "cifs" | "smb-share" => Some("smb"),
        "nfs" | "nfs4" => Some("nfs"),
        "ftp" | "ftps" | "ftpfs" | "curlftpfs" => Some("ftp"),
        "dav" | "webdav" | "davfs2" => Some("dav"),
        "davs" | "webdavs" => Some("davs"),
        "afp" | "afpfs" | "afp-volume" => Some("afp"),
        "http" => Some("http"),
        "https" => Some("https"),
        "mtp" => Some("mtp"),
        _ => None,
    }
}

pub(crate) fn canonical_scheme_or_raw(raw: &str) -> String {
    canonical_scheme(raw)
        .map(str::to_string)
        .unwrap_or_else(|| raw.trim().to_ascii_lowercase())
}

pub(crate) fn canonicalize_uri(uri: &str) -> Option<(String, String)> {
    let trimmed = uri.trim();
    let (raw_scheme, rest) = trimmed.split_once("://")?;
    let scheme = canonical_scheme(raw_scheme)?.to_string();
    let raw_lc = raw_scheme.to_ascii_lowercase();
    let normalized_scheme = match raw_lc.as_str() {
        "ssh" | "sshfs" | "fuse.sshfs" => "sftp",
        "smb3" | "smbfs" | "cifs" | "smb-share" => "smb",
        "nfs4" => "nfs",
        "ftpfs" | "curlftpfs" => "ftp",
        "webdav" | "davfs2" => "dav",
        "webdavs" => "davs",
        "afpfs" | "afp-volume" => "afp",
        _ => raw_lc.as_str(),
    };
    let normalized = if normalized_scheme == raw_lc {
        trimmed.to_string()
    } else {
        format!("{normalized_scheme}://{rest}")
    };
    Some((scheme, normalized))
}

pub(crate) fn normalize_uri_for_compare(uri: &str) -> Option<String> {
    let trimmed = uri.trim();
    let (raw_scheme, rest) = trimmed.split_once("://")?;
    let scheme = canonical_scheme_or_raw(raw_scheme);

    let (authority_raw, path_raw) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, String::new()),
    };
    let authority = normalize_authority_for_compare(authority_raw)?;
    let mut normalized = format!("{scheme}://{authority}{path_raw}");
    while normalized.ends_with('/') {
        normalized.pop();
    }
    Some(normalized)
}

pub(crate) fn classify_uri(uri: &str) -> NetworkUriClassification {
    let trimmed = uri.trim();
    let Some((raw_scheme, _)) = trimmed.split_once("://") else {
        return NetworkUriClassification {
            kind: NetworkUriKind::NotUri,
            scheme: None,
            normalized_uri: None,
        };
    };

    let raw_scheme_lc = raw_scheme.trim().to_ascii_lowercase();
    if raw_scheme_lc.is_empty() {
        return NetworkUriClassification {
            kind: NetworkUriKind::NotUri,
            scheme: None,
            normalized_uri: None,
        };
    }

    let Some((scheme, normalized_uri)) = canonicalize_uri(trimmed) else {
        return NetworkUriClassification {
            kind: NetworkUriKind::Unsupported,
            scheme: Some(raw_scheme_lc),
            normalized_uri: None,
        };
    };

    let kind = if matches!(
        scheme.as_str(),
        "onedrive" | "sftp" | "smb" | "nfs" | "ftp" | "dav" | "davs" | "afp"
    ) {
        NetworkUriKind::Mountable
    } else if matches!(scheme.as_str(), "http" | "https") {
        NetworkUriKind::External
    } else {
        NetworkUriKind::Unsupported
    };

    NetworkUriClassification {
        kind,
        scheme: Some(scheme),
        normalized_uri: Some(normalized_uri),
    }
}

pub(crate) fn resolve_mounted_path_for_uri_in_mounts(
    uri: &str,
    mounts: &[MountInfo],
) -> Option<String> {
    let classified = classify_uri(uri);
    if classified.kind != NetworkUriKind::Mountable {
        return None;
    }
    let scheme = classified.scheme.as_deref()?;

    #[derive(Clone)]
    struct Candidate {
        mount_path: String,
        params: HashMap<String, String>,
        host: Option<String>,
    }

    let mut candidates: Vec<Candidate> = mounts
        .iter()
        .filter_map(|mount| {
            let mount_path = mount.path.trim();
            if is_uri_like(mount_path) {
                return None;
            }
            let fs_scheme = mount_scheme(mount)?;
            if fs_scheme != scheme {
                return None;
            }
            let params = gvfs_params(mount_path);
            let host = normalize_host(
                params
                    .get("host")
                    .or_else(|| params.get("server"))
                    .map(String::as_str)
                    .unwrap_or(""),
            );
            Some(Candidate {
                mount_path: mount.path.clone(),
                params,
                host: if host.is_empty() { None } else { Some(host) },
            })
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    if let Some(uri_host_value) = uri_host(uri) {
        let strict: Vec<Candidate> = candidates
            .iter()
            .filter(|candidate| candidate.host.as_deref() == Some(uri_host_value.as_str()))
            .cloned()
            .collect();
        if !strict.is_empty() {
            candidates = strict;
        }
    }

    if let Some(first_segment) = uri_first_segment(uri) {
        if scheme == "smb" || scheme == "afp" {
            let key = if scheme == "smb" { "share" } else { "volume" };
            let by_segment: Vec<Candidate> = candidates
                .iter()
                .filter(|candidate| {
                    normalize_segment(candidate.params.get(key).map(String::as_str).unwrap_or(""))
                        == first_segment
                })
                .cloned()
                .collect();
            if !by_segment.is_empty() {
                candidates = by_segment;
            }
        }
    }

    if scheme == "nfs" || scheme == "dav" || scheme == "davs" {
        let target_path = normalize_path_for_compare(&uri_path(uri));
        let key = if scheme == "nfs" { "share" } else { "prefix" };
        let by_path: Vec<Candidate> = candidates
            .iter()
            .filter(|candidate| {
                normalize_path_for_compare(
                    candidate.params.get(key).map(String::as_str).unwrap_or(""),
                ) == target_path
            })
            .cloned()
            .collect();
        if !by_path.is_empty() {
            candidates = by_path;
        }
    }

    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.mount_path)
}

#[tauri::command]
pub fn classify_network_uri(uri: String) -> NetworkUriClassification {
    classify_uri(&uri)
}

#[tauri::command]
pub fn resolve_mounted_path_for_uri(uri: String) -> Result<Option<String>, String> {
    let mounts = mounts::list_mounts_sync();
    Ok(resolve_mounted_path_for_uri_in_mounts(&uri, &mounts))
}

fn is_uri_like(value: &str) -> bool {
    value
        .trim()
        .split_once("://")
        .map(|(scheme, _)| !scheme.trim().is_empty())
        .unwrap_or(false)
}

fn normalize_authority_for_compare(authority: &str) -> Option<String> {
    let authority = authority.trim();
    if authority.is_empty() {
        return None;
    }

    let (userinfo, host_port) = match authority.rsplit_once('@') {
        Some((user, host)) if !host.trim().is_empty() => (Some(user.trim()), host.trim()),
        _ => (None, authority),
    };

    let host_port_normalized = if host_port.starts_with('[') {
        let end = host_port.find(']')?;
        let host = host_port[1..end].trim();
        if host.is_empty() {
            return None;
        }
        let rest = host_port[(end + 1)..].trim();
        let host_lc = host.to_ascii_lowercase();
        if rest.is_empty() {
            format!("[{host_lc}]")
        } else if let Some(port) = rest.strip_prefix(':') {
            let port = port.trim();
            if port.is_empty() {
                return None;
            }
            format!("[{host_lc}]:{port}")
        } else {
            return None;
        }
    } else if host_port.matches(':').count() > 1 {
        let host = host_port.trim();
        if host.is_empty() {
            return None;
        }
        host.to_ascii_lowercase()
    } else {
        let (host, port) = match host_port.split_once(':') {
            Some((host, port)) => (host.trim(), Some(port.trim())),
            None => (host_port.trim(), None),
        };
        if host.is_empty() {
            return None;
        }
        let host_lc = host.to_ascii_lowercase();
        match port {
            Some(p) if !p.is_empty() => format!("{host_lc}:{p}"),
            Some(_) => return None,
            None => host_lc,
        }
    };

    Some(match userinfo {
        Some(user) if !user.is_empty() => format!("{user}@{host_port_normalized}"),
        _ => host_port_normalized,
    })
}

fn normalize_host(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
        return trimmed[1..(trimmed.len() - 1)].to_ascii_lowercase();
    }
    if !trimmed.starts_with('[') && trimmed.contains(':') && trimmed.split(':').count() == 2 {
        return trimmed
            .split_once(':')
            .map(|(host, _)| host.trim().to_ascii_lowercase())
            .unwrap_or_default();
    }
    trimmed.to_ascii_lowercase()
}

fn uri_host(value: &str) -> Option<String> {
    let idx = value.find("://")?;
    if idx == 0 {
        return None;
    }
    let remainder = &value[(idx + 3)..];
    if remainder.is_empty() {
        return None;
    }
    let authority = remainder.split('/').next()?.trim();
    if authority.is_empty() {
        return None;
    }
    let without_user = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    if without_user.is_empty() {
        return None;
    }
    if without_user.starts_with('[') {
        if let Some(end) = without_user.find(']') {
            let host = normalize_host(&without_user[1..end]);
            return if host.is_empty() { None } else { Some(host) };
        }
    }
    let host = normalize_host(without_user);
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn uri_path(value: &str) -> String {
    let Some(idx) = value.find("://") else {
        return String::new();
    };
    if idx == 0 {
        return String::new();
    }
    let remainder = &value[(idx + 3)..];
    if remainder.is_empty() {
        return String::new();
    }
    let Some(slash) = remainder.find('/') else {
        return String::new();
    };
    remainder[(slash + 1)..].to_string()
}

fn normalize_path_for_compare(value: &str) -> String {
    let decoded = safe_percent_decode(value).trim().to_string();
    if decoded.is_empty() {
        return "/".to_string();
    }
    let with_leading = if decoded.starts_with('/') {
        decoded
    } else {
        format!("/{decoded}")
    };
    let collapsed = with_leading.trim_end_matches('/');
    if collapsed.is_empty() {
        "/".to_string()
    } else {
        collapsed.to_ascii_lowercase()
    }
}

fn normalize_segment(value: &str) -> String {
    safe_percent_decode(value)
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn gvfs_entry_name(path: &str) -> Option<String> {
    let normalized_path = path.replace('\\', "/");
    let marker = "/gvfs/";
    let idx = normalized_path.to_ascii_lowercase().find(marker)?;
    let entry = normalized_path[(idx + marker.len())..]
        .split('/')
        .next()
        .unwrap_or_default()
        .trim();
    if entry.is_empty() {
        None
    } else {
        Some(entry.to_string())
    }
}

fn gvfs_params(path: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(entry) = gvfs_entry_name(path) else {
        return out;
    };
    let Some(args_idx) = entry.find(':') else {
        return out;
    };
    if args_idx >= entry.len() - 1 {
        return out;
    }
    for token in entry[(args_idx + 1)..].split(',') {
        let Some((raw_key, raw_value)) = token.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        out.insert(key, safe_percent_decode(raw_value.trim()));
    }
    out
}

fn mount_scheme(mount: &MountInfo) -> Option<String> {
    if let Some(from_fs) = canonical_scheme(&mount.fs) {
        return Some(from_fs.to_string());
    }
    let entry = gvfs_entry_name(&mount.path)?;
    let prefix = entry.split(':').next().unwrap_or_default();
    canonical_scheme(prefix).map(str::to_string)
}

fn uri_first_segment(value: &str) -> Option<String> {
    let path = uri_path(value);
    if path.is_empty() {
        return None;
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        let normalized = normalize_segment(segment);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }
    None
}

fn safe_percent_decode(value: &str) -> String {
    fn hex_value(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_uri_detects_mountable_external_and_unsupported() {
        let mountable = classify_uri("ssh://alice@nas.local/share");
        assert_eq!(mountable.kind, NetworkUriKind::Mountable);
        assert_eq!(mountable.scheme.as_deref(), Some("sftp"));
        assert_eq!(
            mountable.normalized_uri.as_deref(),
            Some("sftp://alice@nas.local/share")
        );

        let external = classify_uri("https://example.com");
        assert_eq!(external.kind, NetworkUriKind::External);
        assert_eq!(external.scheme.as_deref(), Some("https"));

        let unsupported = classify_uri("ftps+special://example.com");
        assert_eq!(unsupported.kind, NetworkUriKind::Unsupported);
        assert_eq!(unsupported.scheme.as_deref(), Some("ftps+special"));
    }

    #[test]
    fn normalize_uri_for_compare_maps_aliases_and_trims_slashes() {
        assert_eq!(
            normalize_uri_for_compare("SSH://alice@EXAMPLE.com:2222/"),
            Some("sftp://alice@example.com:2222".to_string())
        );
        assert_eq!(
            normalize_uri_for_compare("FTPS://example.com/path/"),
            Some("ftp://example.com/path".to_string())
        );
        assert_eq!(
            normalize_uri_for_compare("webdav://Nas.LOCAL/share/"),
            Some("dav://nas.local/share".to_string())
        );
        assert_eq!(
            normalize_uri_for_compare("webdavs://[2001:DB8::1]:8443/path///"),
            Some("davs://[2001:db8::1]:8443/path".to_string())
        );
    }

    #[test]
    fn resolve_mounted_path_matches_host_and_share() {
        let mounts = vec![
            MountInfo {
                label: "NAS A".into(),
                path: "/run/user/1000/gvfs/smb-share:server=nas-a.local,share=files".into(),
                fs: "smb".into(),
                removable: false,
            },
            MountInfo {
                label: "NAS B".into(),
                path: "/run/user/1000/gvfs/smb-share:server=nas-b.local,share=files".into(),
                fs: "smb".into(),
                removable: false,
            },
        ];

        let resolved = resolve_mounted_path_for_uri_in_mounts("smb://NAS-B.local/files", &mounts);
        assert_eq!(
            resolved.as_deref(),
            Some("/run/user/1000/gvfs/smb-share:server=nas-b.local,share=files")
        );
    }
}

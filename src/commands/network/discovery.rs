use crate::commands::fs::MountInfo;
use once_cell::sync::OnceCell;
use std::collections::HashSet;
#[cfg(not(target_os = "windows"))]
use std::net::UdpSocket;
#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use url::Url;

use super::uri::canonicalize_uri;

const DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(20);

fn instant_ago(d: Duration) -> Instant {
    Instant::now().checked_sub(d).unwrap_or_else(Instant::now)
}

fn scheme_label(scheme: &str) -> &'static str {
    match scheme {
        "onedrive" => "OneDrive",
        "sftp" => "SFTP",
        "smb" => "SMB",
        "nfs" => "NFS",
        "ftp" => "FTP",
        "dav" => "WebDAV",
        "davs" => "WebDAVS",
        "afp" => "AFP",
        "http" => "HTTP",
        "https" => "HTTPS",
        _ => "Network",
    }
}

fn host_from_uri(uri: &str) -> Option<String> {
    if let Ok(url) = Url::parse(uri) {
        if let Some(host) = url.host_str() {
            let trimmed = host.trim().trim_end_matches('.');
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    let (_, rest) = uri.split_once("://")?;
    let authority = rest.split('/').next()?.trim();
    if authority.is_empty() {
        return None;
    }
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = if authority.starts_with('[') {
        if let Some(end) = authority.find(']') {
            &authority[1..end]
        } else {
            authority
        }
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    let trimmed = host.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn default_label(name: Option<&str>, scheme: &str, uri: &str) -> String {
    let explicit = name.unwrap_or("").trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    if let Some(host) = host_from_uri(uri) {
        return format!("{} ({host})", scheme_label(scheme));
    }
    scheme_label(scheme).to_string()
}

fn mount_info(label: String, path: String, fs: &str) -> MountInfo {
    MountInfo {
        label,
        path,
        fs: fs.to_string(),
        removable: false,
    }
}

#[cfg(not(target_os = "windows"))]
fn parse_gio_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !(trimmed.starts_with("Volume(") || trimmed.starts_with("Mount(")) {
        return None;
    }
    let (_, rest) = trimmed.split_once(':')?;
    let raw = rest.split("->").next().unwrap_or(rest).trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn list_from_gio() -> Vec<MountInfo> {
    let mut out = Vec::new();
    let output = match Command::new("gio")
        .arg("mount")
        .arg("-li")
        .stderr(Stdio::null())
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return out,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_name: Option<String> = None;

    for line in stdout.lines() {
        let l = line.trim();
        if let Some(name) = parse_gio_name(l) {
            current_name = Some(name);
            continue;
        }

        let uri = l
            .strip_prefix("activation_root=")
            .or_else(|| l.strip_prefix("default_location="));
        let Some(uri_raw) = uri else { continue };
        let Some((scheme, normalized)) = canonicalize_uri(uri_raw) else {
            continue;
        };
        let label = default_label(current_name.as_deref(), &scheme, &normalized);
        out.push(mount_info(label, normalized, &scheme));
    }

    out
}

#[cfg(not(target_os = "windows"))]
fn scheme_for_service_type(service_type: &str) -> Option<&'static str> {
    match service_type.to_ascii_lowercase().as_str() {
        "_sftp-ssh._tcp" | "_ssh._tcp" => Some("sftp"),
        "_smb._tcp" => Some("smb"),
        "_nfs._tcp" => Some("nfs"),
        "_ftp._tcp" => Some("ftp"),
        "_webdav._tcp" => Some("dav"),
        "_webdavs._tcp" => Some("davs"),
        "_afpovertcp._tcp" => Some("afp"),
        "_http._tcp" => Some("http"),
        "_https._tcp" => Some("https"),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        "sftp" => Some(22),
        "smb" => Some(445),
        "nfs" => Some(2049),
        "ftp" => Some(21),
        "dav" => Some(80),
        "davs" => Some(443),
        "afp" => Some(548),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn format_host_for_uri(host: &str) -> String {
    let trimmed = host.trim().trim_end_matches('.');
    if trimmed.contains(':') && !trimmed.starts_with('[') && !trimmed.ends_with(']') {
        format!("[{trimmed}]")
    } else {
        trimmed.to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn build_uri(scheme: &str, host: &str, port: Option<u16>) -> Option<String> {
    let host = format_host_for_uri(host);
    if host.is_empty() {
        return None;
    }
    let include_port = match (port, default_port_for_scheme(scheme)) {
        (Some(p), Some(default)) if p != default => Some(p),
        (Some(p), None) => Some(p),
        _ => None,
    };
    Some(match include_port {
        Some(p) => format!("{scheme}://{host}:{p}/"),
        None => format!("{scheme}://{host}/"),
    })
}

#[cfg(not(target_os = "windows"))]
fn list_from_avahi() -> Vec<MountInfo> {
    let mut out = Vec::new();
    let output = match Command::new("avahi-browse")
        .args(["-a", "-r", "-t", "-p"])
        .stderr(Stdio::null())
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return out,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let cols: Vec<&str> = line.split(';').collect();
        if cols.len() < 9 || cols[0] != "=" {
            continue;
        }
        let service_name = cols[3].trim();
        let service_type = cols[4].trim();
        let Some(scheme) = scheme_for_service_type(service_type) else {
            continue;
        };
        let host = cols[6].trim();
        let addr = cols[7].trim();
        let target = if !host.is_empty() { host } else { addr };
        let port = cols[8].trim().parse::<u16>().ok();
        let Some(uri) = build_uri(scheme, target, port) else {
            continue;
        };
        let label = default_label(Some(service_name), scheme, &uri);
        out.push(mount_info(label, uri, scheme));
    }

    out
}

#[cfg(not(target_os = "windows"))]
fn parse_ssdp_headers(response: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut location: Option<String> = None;
    let mut server: Option<String> = None;
    let mut st: Option<String> = None;
    for line in response.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key_lc = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key_lc.as_str() {
            "location" => location = Some(value.to_string()),
            "server" => server = Some(value.to_string()),
            "st" => st = Some(value.to_string()),
            _ => {}
        }
    }
    (location, server, st)
}

#[cfg(not(target_os = "windows"))]
fn list_from_ssdp() -> Vec<MountInfo> {
    let mut out = Vec::new();
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return out,
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
    let req = concat!(
        "M-SEARCH * HTTP/1.1\r\n",
        "HOST:239.255.255.250:1900\r\n",
        "MAN:\"ssdp:discover\"\r\n",
        "MX:1\r\n",
        "ST:ssdp:all\r\n",
        "\r\n"
    );
    let _ = socket.send_to(req.as_bytes(), "239.255.255.250:1900");

    let deadline = Instant::now() + Duration::from_millis(1200);
    let mut buf = [0u8; 8192];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                let (location, server, st) = parse_ssdp_headers(&text);
                let Some(location) = location else { continue };
                let Some((scheme, normalized)) = canonicalize_uri(&location) else {
                    continue;
                };
                let source = server.or(st);
                let label = default_label(source.as_deref(), &scheme, &normalized);
                out.push(mount_info(label, normalized, &scheme));
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                break;
            }
            Err(_) => break,
        }
    }

    out
}

fn dedupe_and_sort(mut list: Vec<MountInfo>) -> Vec<MountInfo> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in list.drain(..) {
        let key = format!(
            "{}|{}",
            item.fs.to_ascii_lowercase(),
            item.path.trim_end_matches('/').to_ascii_lowercase()
        );
        if seen.insert(key) {
            out.push(item);
        }
    }
    out.sort_by(|a, b| {
        a.label
            .to_ascii_lowercase()
            .cmp(&b.label.to_ascii_lowercase())
            .then(a.path.cmp(&b.path))
    });
    out
}

#[cfg(not(target_os = "windows"))]
pub(super) fn list_network_devices_sync() -> Vec<MountInfo> {
    static CACHE: OnceCell<Mutex<(Instant, Vec<MountInfo>)>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new((instant_ago(DISCOVERY_CACHE_TTL), Vec::new())));

    if let Ok(guard) = cache.lock() {
        if guard.0.elapsed() < DISCOVERY_CACHE_TTL {
            return guard.1.clone();
        }
    }

    let mut combined = Vec::new();
    combined.extend(list_from_gio());
    combined.extend(list_from_avahi());
    combined.extend(list_from_ssdp());
    let result = dedupe_and_sort(combined);

    if let Ok(mut guard) = cache.lock() {
        *guard = (Instant::now(), result.clone());
    }
    result
}

#[cfg(target_os = "windows")]
pub(super) fn list_network_devices_sync() -> Vec<MountInfo> {
    Vec::new()
}

#[tauri::command]
pub async fn list_network_devices() -> Result<Vec<MountInfo>, String> {
    tauri::async_runtime::spawn_blocking(list_network_devices_sync)
        .await
        .map_err(|e| format!("network discovery failed: {e}"))
}

#[tauri::command]
pub fn open_network_uri(uri: String) -> Result<(), String> {
    let Some((scheme, normalized)) = canonicalize_uri(&uri) else {
        return Err("Unsupported URI".into());
    };
    if !matches!(scheme.as_str(), "http" | "https") {
        return Err("Only HTTP/HTTPS URIs are supported".into());
    }
    let parsed = Url::parse(&normalized).map_err(|e| format!("Invalid URI: {e}"))?;
    open::that_detached(parsed.as_str()).map_err(|e| format!("Failed to open URI: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_uri_maps_alias_schemes() {
        let (scheme, uri) = canonicalize_uri("ssh://user@host/").expect("ssh should map");
        assert_eq!(scheme, "sftp");
        assert_eq!(uri, "sftp://user@host/");

        let (scheme, uri) = canonicalize_uri("webdavs://host/share").expect("webdavs should map");
        assert_eq!(scheme, "davs");
        assert_eq!(uri, "davs://host/share");

        let (scheme, uri) = canonicalize_uri("ftps://host/path").expect("ftps should map");
        assert_eq!(scheme, "ftp");
        assert_eq!(uri, "ftps://host/path");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn parse_ssdp_headers_extracts_location_and_server() {
        let payload = concat!(
            "HTTP/1.1 200 OK\r\n",
            "LOCATION: http://192.168.1.10:80/desc.xml\r\n",
            "SERVER: Fibaro/1.0 UPnP/1.1\r\n",
            "ST: upnp:rootdevice\r\n",
            "\r\n"
        );
        let (location, server, st) = parse_ssdp_headers(payload);
        assert_eq!(location.as_deref(), Some("http://192.168.1.10:80/desc.xml"));
        assert_eq!(server.as_deref(), Some("Fibaro/1.0 UPnP/1.1"));
        assert_eq!(st.as_deref(), Some("upnp:rootdevice"));
    }
}

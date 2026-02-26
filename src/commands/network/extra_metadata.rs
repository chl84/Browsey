//! Network URI metadata helpers used by the properties modal.

use crate::metadata::types::{ExtraMetadataField, ExtraMetadataResult, ExtraMetadataSection};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedUri {
    protocol: String,
    user: Option<String>,
    host: Option<String>,
    port: Option<String>,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
}

pub(crate) fn looks_like_uri_path(path: &str) -> bool {
    let trimmed = path.trim();
    let Some(idx) = trimmed.find("://") else {
        return false;
    };
    if idx == 0 {
        return false;
    }

    let scheme = &trimmed[..idx];
    let mut chars = scheme.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

fn parse_uri_for_extra(value: &str) -> Option<ParsedUri> {
    let trimmed = value.trim();
    let idx = trimmed.find("://")?;
    if idx == 0 {
        return None;
    }

    let protocol = trimmed[..idx].to_ascii_lowercase();
    let rest = &trimmed[(idx + 3)..];

    let split_at = rest.find(['/', '?', '#']);
    let authority = match split_at {
        Some(pos) => rest[..pos].trim(),
        None => rest.trim(),
    };
    let mut tail = split_at.map(|pos| &rest[pos..]).unwrap_or_default();

    let mut fragment: Option<String> = None;
    if let Some(hash_idx) = tail.find('#') {
        let decoded = safe_percent_decode(tail[(hash_idx + 1)..].trim());
        fragment = if decoded.is_empty() {
            None
        } else {
            Some(decoded)
        };
        tail = &tail[..hash_idx];
    }

    let mut query: Option<String> = None;
    if let Some(query_idx) = tail.find('?') {
        let decoded = safe_percent_decode(tail[(query_idx + 1)..].trim());
        query = if decoded.is_empty() {
            None
        } else {
            Some(decoded)
        };
        tail = &tail[..query_idx];
    }

    let decoded_path = safe_percent_decode(tail.trim());
    let path = if decoded_path.is_empty() {
        None
    } else {
        Some(decoded_path)
    };

    let (user, host_port) = match authority.rsplit_once('@') {
        Some((left, right)) => {
            let decoded_user = safe_percent_decode(left.trim());
            (
                if decoded_user.is_empty() {
                    None
                } else {
                    Some(decoded_user)
                },
                right.trim(),
            )
        }
        None => (None, authority),
    };

    let (host, port) = if host_port.starts_with('[') {
        if let Some(end) = host_port.find(']') {
            let decoded_host = safe_percent_decode(host_port[1..end].trim());
            let rest_after = host_port[(end + 1)..].trim();
            let port = rest_after
                .strip_prefix(':')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            (
                if decoded_host.is_empty() {
                    None
                } else {
                    Some(decoded_host)
                },
                port,
            )
        } else {
            let decoded_host = safe_percent_decode(host_port.trim());
            (
                if decoded_host.is_empty() {
                    None
                } else {
                    Some(decoded_host)
                },
                None,
            )
        }
    } else if host_port.matches(':').count() == 1 {
        let (raw_host, raw_port) = host_port.split_once(':').unwrap_or((host_port, ""));
        let decoded_host = safe_percent_decode(raw_host.trim());
        let port = raw_port.trim();
        (
            if decoded_host.is_empty() {
                None
            } else {
                Some(decoded_host)
            },
            if port.is_empty() {
                None
            } else {
                Some(port.to_string())
            },
        )
    } else {
        let decoded_host = safe_percent_decode(host_port.trim());
        (
            if decoded_host.is_empty() {
                None
            } else {
                Some(decoded_host)
            },
            None,
        )
    };

    Some(ParsedUri {
        protocol,
        user,
        host,
        port,
        path,
        query,
        fragment,
    })
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

pub(crate) fn build_network_uri_extra_metadata(path: &str) -> ExtraMetadataResult {
    let mut fields: Vec<ExtraMetadataField> = vec![ExtraMetadataField::new(
        "address",
        "Address",
        path.to_string(),
    )];

    if let Some(parsed) = parse_uri_for_extra(path) {
        fields.push(ExtraMetadataField::new(
            "protocol",
            "Protocol",
            parsed.protocol.to_ascii_uppercase(),
        ));
        if let Some(user) = parsed.user {
            fields.push(ExtraMetadataField::new("user", "User", user));
        }
        if let Some(host) = parsed.host {
            fields.push(ExtraMetadataField::new("host", "Host", host));
        }
        if let Some(port) = parsed.port {
            fields.push(ExtraMetadataField::new("port", "Port", port));
        }
        if let Some(path_value) = parsed.path {
            fields.push(ExtraMetadataField::new("path", "Path", path_value));
        }
        if let Some(query) = parsed.query {
            fields.push(ExtraMetadataField::new("query", "Query", query));
        }
        if let Some(fragment) = parsed.fragment {
            fields.push(ExtraMetadataField::new("fragment", "Fragment", fragment));
        }
    }

    ExtraMetadataResult {
        kind: "network-uri".to_string(),
        sections: vec![ExtraMetadataSection::new("network-uri", "Network").with_fields(fields)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_for_key<'a>(result: &'a ExtraMetadataResult, key: &str) -> Option<&'a str> {
        result.sections.first().and_then(|section| {
            section
                .fields
                .iter()
                .find(|field| field.key == key)
                .map(|field| field.value.as_str())
        })
    }

    #[test]
    fn looks_like_uri_path_checks_scheme_shape() {
        assert!(looks_like_uri_path("sftp://host/path"));
        assert!(looks_like_uri_path("dav://server/share"));
        assert!(!looks_like_uri_path("://broken"));
        assert!(!looks_like_uri_path("/home/chris"));
    }

    #[test]
    fn build_network_uri_extra_metadata_parses_standard_fields() {
        let result = build_network_uri_extra_metadata(
            "sftp://alice@example.local:2222/home/alice/docs?q=a%2Bb#part%201",
        );
        assert_eq!(result.kind, "network-uri");
        assert_eq!(value_for_key(&result, "protocol"), Some("SFTP"));
        assert_eq!(value_for_key(&result, "user"), Some("alice"));
        assert_eq!(value_for_key(&result, "host"), Some("example.local"));
        assert_eq!(value_for_key(&result, "port"), Some("2222"));
        assert_eq!(value_for_key(&result, "path"), Some("/home/alice/docs"));
        assert_eq!(value_for_key(&result, "query"), Some("q=a+b"));
        assert_eq!(value_for_key(&result, "fragment"), Some("part 1"));
    }

    #[test]
    fn build_network_uri_extra_metadata_uses_host_label_for_network_uri() {
        let result = build_network_uri_extra_metadata("dav://files.example.local/Documents");
        assert_eq!(value_for_key(&result, "protocol"), Some("DAV"));
        assert_eq!(value_for_key(&result, "host"), Some("files.example.local"));
        assert_eq!(value_for_key(&result, "path"), Some("/Documents"));
    }
}

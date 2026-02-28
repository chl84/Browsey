use super::{
    daemon::{daemon_is_running, kill_daemon, rclone_rc_state, spawn_daemon},
    RcloneCliError, RcloneRcClient, RcloneRcMethod, RcloneSubcommand,
    RCLONE_RC_ERROR_TEXT_MAX_CHARS, RCLONE_RC_START_FAILURE_COOLDOWN,
};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    io,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Instant,
};
use tracing::{info, warn};

impl RcloneRcClient {
    pub(super) fn recycle_daemon_after_error(
        &self,
        method: RcloneRcMethod,
        error: &RcloneCliError,
    ) {
        let mut state = match rclone_rc_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(mut daemon) = state.daemon.take() {
            let socket = daemon.socket_path.display().to_string();
            let _ = kill_daemon(&mut daemon);
            warn!(
                method = method.as_str(),
                error = %error,
                socket = %socket,
                "recycled rclone rcd daemon after transport-level rc failure"
            );
        }
    }

    pub(super) fn ensure_daemon_ready(&self) -> Result<PathBuf, RcloneCliError> {
        if !self.is_enabled() {
            return Err(RcloneCliError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "rclone rc backend is disabled",
            )));
        }

        let mut state = match rclone_rc_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        if let Some(until) = state.startup_blocked_until {
            let binary_matches = state
                .startup_blocked_binary
                .as_ref()
                .map(|blocked| blocked == &self.binary)
                .unwrap_or(false);
            if until > now && binary_matches {
                return Err(RcloneCliError::Io(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "rclone rc startup cooldown active ({}ms remaining)",
                        until.saturating_duration_since(now).as_millis() as u64
                    ),
                )));
            }
            state.startup_blocked_until = None;
            state.startup_blocked_binary = None;
        }

        if let Some(existing) = state.daemon.as_mut() {
            if existing.binary != self.binary {
                info!(
                    existing_binary = %existing.binary.to_string_lossy(),
                    requested_binary = %self.binary.to_string_lossy(),
                    "restarting rclone rcd daemon for different binary path"
                );
                let _ = kill_daemon(existing);
                state.daemon = None;
            }
        }

        if let Some(existing) = state.daemon.as_mut() {
            if daemon_is_running(existing)? && existing.socket_path.exists() {
                return Ok(existing.socket_path.clone());
            }
            info!(
                socket = %existing.socket_path.display(),
                "restarting stale rclone rcd daemon"
            );
            let _ = kill_daemon(existing);
            state.daemon = None;
        }

        let daemon = match spawn_daemon(&self.binary) {
            Ok(daemon) => daemon,
            Err(error) => {
                let cooldown_until = Instant::now() + RCLONE_RC_START_FAILURE_COOLDOWN;
                state.startup_blocked_until = Some(cooldown_until);
                state.startup_blocked_binary = Some(self.binary.clone());
                warn!(
                    cooldown_ms = RCLONE_RC_START_FAILURE_COOLDOWN.as_millis() as u64,
                    error = %error,
                    "rclone rcd startup failed; applying startup cooldown"
                );
                return Err(error);
            }
        };
        let socket_path = daemon.socket_path.clone();
        info!(socket = %socket_path.display(), "started rclone rcd daemon");
        state.startup_blocked_until = None;
        state.startup_blocked_binary = None;
        state.daemon = Some(daemon);
        Ok(socket_path)
    }
}

pub(super) fn run_rc_command_via_socket(
    socket_path: &Path,
    method: RcloneRcMethod,
    payload: Value,
    timeout: std::time::Duration,
) -> Result<Value, RcloneCliError> {
    if !cfg!(unix) {
        return Err(RcloneCliError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "rclone rc unix socket transport is only supported on unix targets",
        )));
    }
    let payload_text = serde_json::to_string(&payload).map_err(|error| {
        RcloneCliError::Io(io::Error::other(format!(
            "failed to encode rclone rc payload: {error}"
        )))
    })?;

    let response_text =
        send_rc_http_request_over_unix_socket(socket_path, method, &payload_text, timeout)?;
    let body = parse_http_response_body(&response_text, method)?;
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(&body).map_err(|error| {
        RcloneCliError::Io(io::Error::other(format!(
            "invalid JSON from rclone rc {}: {error}",
            method.as_str()
        )))
    })
}

#[cfg(unix)]
fn send_rc_http_request_over_unix_socket(
    socket_path: &Path,
    method: RcloneRcMethod,
    payload_text: &str,
    timeout: std::time::Duration,
) -> Result<String, RcloneCliError> {
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path)
        .map_err(|error| map_rc_io_error(error, timeout, "connect"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(RcloneCliError::Io)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(RcloneCliError::Io)?;

    let request = format!(
        "POST /{} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method.as_str(),
        payload_text.len(),
        payload_text
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|error| map_rc_io_error(error, timeout, "write"))?;

    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(|error| map_rc_io_error(error, timeout, "read"))?;
    Ok(String::from_utf8_lossy(&response_bytes).to_string())
}

#[cfg(not(unix))]
fn send_rc_http_request_over_unix_socket(
    _socket_path: &Path,
    _method: RcloneRcMethod,
    _payload_text: &str,
    _timeout: std::time::Duration,
) -> Result<String, RcloneCliError> {
    Err(RcloneCliError::Io(io::Error::new(
        io::ErrorKind::Unsupported,
        "rclone rc unix socket transport is only supported on unix targets",
    )))
}

fn map_rc_io_error(error: io::Error, timeout: std::time::Duration, phase: &str) -> RcloneCliError {
    if error.kind() == io::ErrorKind::TimedOut || error.kind() == io::ErrorKind::WouldBlock {
        RcloneCliError::Timeout {
            subcommand: RcloneSubcommand::Rc,
            timeout,
            stdout: String::new(),
            stderr: format!("rclone rc socket {phase} timed out"),
        }
    } else {
        RcloneCliError::Io(error)
    }
}

fn parse_http_response_body(
    response_text: &str,
    method: RcloneRcMethod,
) -> Result<String, RcloneCliError> {
    let Some((header, body)) = response_text.split_once("\r\n\r\n") else {
        return Err(RcloneCliError::Io(io::Error::other(format!(
            "invalid HTTP response from rclone rc {}",
            method.as_str()
        ))));
    };

    let mut lines = header.lines();
    let status_line = lines.next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse::<u16>().ok())
        .unwrap_or(0);
    let chunked = lines.any(|line| {
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or_default().trim();
        let value = parts.next().unwrap_or_default().trim();
        name.eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
    });
    let decoded_body = if chunked {
        decode_chunked_http_body(body, method)?
    } else {
        body.to_string()
    };
    if !(200..300).contains(&status_code) {
        let body_scrubbed = scrub_rc_error_text(decoded_body.trim());
        return Err(RcloneCliError::Io(io::Error::other(format!(
            "rclone rc {} failed with HTTP {} ({})",
            method.as_str(),
            status_code,
            body_scrubbed
        ))));
    }
    Ok(decoded_body)
}

fn decode_chunked_http_body(body: &str, method: RcloneRcMethod) -> Result<String, RcloneCliError> {
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    let mut out = Vec::<u8>::new();

    loop {
        let Some(line_end_offset) = bytes[cursor..].windows(2).position(|w| w == b"\r\n") else {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: missing chunk size line terminator",
                method.as_str()
            ))));
        };
        let line_end = cursor + line_end_offset;
        let size_line = std::str::from_utf8(&bytes[cursor..line_end]).map_err(|error| {
            RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: non-utf8 chunk size: {error}",
                method.as_str()
            )))
        })?;
        let size_hex = size_line.split(';').next().unwrap_or_default().trim();
        let chunk_size = usize::from_str_radix(size_hex, 16).map_err(|error| {
            RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: bad chunk size `{size_hex}`: {error}",
                method.as_str()
            )))
        })?;
        cursor = line_end + 2;

        if chunk_size == 0 {
            break;
        }

        let chunk_end = cursor.saturating_add(chunk_size);
        if chunk_end + 2 > bytes.len() {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: truncated chunk payload",
                method.as_str()
            ))));
        }

        out.extend_from_slice(&bytes[cursor..chunk_end]);
        if &bytes[chunk_end..chunk_end + 2] != b"\r\n" {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: missing chunk terminator",
                method.as_str()
            ))));
        }
        cursor = chunk_end + 2;
    }

    String::from_utf8(out).map_err(|error| {
        RcloneCliError::Io(io::Error::other(format!(
            "invalid UTF-8 body from chunked rclone rc {} response: {error}",
            method.as_str()
        )))
    })
}

fn scrub_rc_error_text(raw: &str) -> String {
    let mut out = raw.to_string();
    for re in sensitive_json_regexes() {
        out = re.replace_all(&out, "$1\"***\"").to_string();
    }
    if out.chars().count() > RCLONE_RC_ERROR_TEXT_MAX_CHARS {
        out = out
            .chars()
            .take(RCLONE_RC_ERROR_TEXT_MAX_CHARS)
            .collect::<String>();
        out.push_str("… [truncated]");
    }
    out
}

fn sensitive_json_regexes() -> &'static Vec<Regex> {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        [
            r#"("access_token"\s*:\s*)"[^"]*""#,
            r#"("refresh_token"\s*:\s*)"[^"]*""#,
            r#"("token"\s*:\s*)"[^"]*""#,
            r#"("pass(word)?"\s*:\s*)"[^"]*""#,
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("valid sensitive JSON regex"))
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_http_response_body, run_rc_command_via_socket, scrub_rc_error_text};
    use crate::commands::cloud::rclone_rc::RcloneRcMethod;
    use serde_json::{json, Value};
    use std::time::Duration;

    #[cfg(unix)]
    use std::{
        fs,
        io::{Read, Write},
        os::unix::net::UnixListener,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    fn unique_test_dir(label: &str) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let unique = format!(
            "browsey-rclone-rc-test-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
                + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).expect("create temp test dir");
        root
    }

    #[cfg(unix)]
    fn spawn_fake_rc_http_server(
        response: &'static str,
        delay: Duration,
    ) -> (PathBuf, PathBuf, thread::JoinHandle<()>) {
        let root = unique_test_dir("sock");
        let socket_path = root.join("rc.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind unix socket");
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                if !delay.is_zero() {
                    thread::sleep(delay);
                }
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (root, socket_path, handle)
    }

    #[test]
    fn parses_success_http_response_body() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let body = parse_http_response_body(response, RcloneRcMethod::CoreNoop).expect("body");
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn parses_chunked_success_http_response_body() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Transfer-Encoding: chunked\r\n",
            "\r\n",
            "4\r\n",
            "{\"ok\r\n",
            "7\r\n",
            "\":true}\r\n",
            "0\r\n",
            "\r\n"
        );
        let body = parse_http_response_body(response, RcloneRcMethod::CoreNoop).expect("body");
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn rejects_non_2xx_http_response() {
        let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\n\r\n{\"error\":\"boom\",\"access_token\":\"abc\",\"token\":\"def\"}";
        let err = parse_http_response_body(response, RcloneRcMethod::CoreNoop).expect_err("err");
        assert!(
            err.to_string().contains("HTTP 500"),
            "unexpected error text: {err}"
        );
        assert!(
            !err.to_string().contains("\"abc\"") && !err.to_string().contains("\"def\""),
            "sensitive values should be redacted: {err}"
        );
    }

    #[test]
    fn scrubs_and_truncates_sensitive_rc_error_text() {
        let raw = format!(
            "{{\"access_token\":\"a\",\"refresh_token\":\"b\",\"pass\":\"c\",\"token\":\"d\",\"pad\":\"{}\"}}",
            "x".repeat(3000)
        );
        let scrubbed = scrub_rc_error_text(&raw);
        assert!(scrubbed.contains("\"access_token\":\"***\""));
        assert!(scrubbed.contains("\"refresh_token\":\"***\""));
        assert!(scrubbed.contains("\"pass\":\"***\""));
        assert!(scrubbed.contains("\"token\":\"***\""));
        assert!(scrubbed.contains("[truncated]"));
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_success_path_parses_json_body() {
        let response =
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true,\"n\":1}";
        let (root, socket_path, handle) =
            spawn_fake_rc_http_server(response, Duration::from_millis(0));
        let value = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({"ping":"pong"}),
            Duration::from_secs(1),
        )
        .expect("rc success response");
        assert_eq!(value.get("ok"), Some(&Value::Bool(true)));
        assert_eq!(value.get("n"), Some(&Value::from(1)));
        handle.join().expect("join fake server");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_returns_timeout_when_server_stalls() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let (root, socket_path, handle) =
            spawn_fake_rc_http_server(response, Duration::from_millis(220));
        let err = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({}),
            Duration::from_millis(40),
        )
        .expect_err("expected timeout");
        assert!(
            matches!(
                err,
                crate::commands::cloud::rclone_cli::RcloneCliError::Timeout { .. }
            ),
            "unexpected error: {err}"
        );
        handle.join().expect("join fake server");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_rejects_malformed_json_payload() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\nnot-json";
        let (root, socket_path, handle) =
            spawn_fake_rc_http_server(response, Duration::from_millis(0));
        let err = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({}),
            Duration::from_secs(1),
        )
        .expect_err("expected malformed payload failure");
        assert!(
            err.to_string().contains("invalid JSON"),
            "unexpected error: {err}"
        );
        handle.join().expect("join fake server");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_fails_cleanly_when_socket_is_unavailable() {
        let root = unique_test_dir("missing");
        let socket_path = root.join("missing.sock");
        let err = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({}),
            Duration::from_millis(60),
        )
        .expect_err("expected unavailable socket error");
        assert!(
            matches!(
                err,
                crate::commands::cloud::rclone_cli::RcloneCliError::Io(_)
            ),
            "unexpected error: {err}"
        );
        let _ = fs::remove_dir_all(root);
    }
}

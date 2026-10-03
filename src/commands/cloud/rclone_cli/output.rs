const RCLONE_FAILURE_OUTPUT_MAX_CHARS: usize = 16 * 1024;

fn redact_urls(raw: &str) -> std::borrow::Cow<'_, str> {
    static URL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    // OData upload-session URLs legitimately contain apostrophes in their path.
    let url = URL.get_or_init(|| regex::Regex::new(r#"(?i)https?://[^\s\"<>]+"#).unwrap());
    url.replace_all(raw, "[redacted URL]")
}

fn redact_json_secrets(raw: &str) -> std::borrow::Cow<'_, str> {
    static SECRET: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = SECRET.get_or_init(|| regex::Regex::new(
        r#"(?i)("(?:access_token|refresh_token|token|pass(?:word)?|secret|authorization)"\s*:\s*)"(?:\\.|[^"\\])*""#).unwrap());
    pattern.replace_all(raw, "$1\"***\"")
}

pub(super) fn scrub_log_text(raw: &str) -> String {
    const MAX_CHARS: usize = 320;
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let raw = redact_urls(raw);
    for (idx, line) in raw.lines().enumerate() {
        if idx > 0 {
            out.push_str(" | ");
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("token")
            || lower.contains("secret")
            || lower.contains("password")
            || lower.contains("authorization")
        {
            out.push_str("[redacted]");
        } else {
            out.push_str(line.trim());
        }
        if out.chars().count() >= MAX_CHARS {
            let mut truncated = out.chars().take(MAX_CHARS).collect::<String>();
            truncated.push('…');
            return truncated;
        }
    }
    out
}

pub(super) fn truncate_failure_output(raw: String) -> String {
    // Failed OneDrive uploads can include signed URLs (including path tokens).
    // Redact before shortening, so an isolated token fragment cannot survive
    // at a cut boundary. Successful JSON/config output must remain untouched.
    let raw = redact_urls(&raw);
    let raw = redact_json_secrets(&raw);
    if raw.chars().count() <= RCLONE_FAILURE_OUTPUT_MAX_CHARS {
        return raw.into_owned();
    }
    let mut truncated = raw
        .chars()
        .take(RCLONE_FAILURE_OUTPUT_MAX_CHARS / 2)
        .collect::<String>();
    truncated.push_str("… [truncated]");
    let tail = raw
        .chars()
        .rev()
        .take(RCLONE_FAILURE_OUTPUT_MAX_CHARS / 2)
        .collect::<Vec<_>>();
    truncated.extend(tail.into_iter().rev());
    truncated
}

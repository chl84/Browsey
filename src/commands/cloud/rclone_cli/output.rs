const RCLONE_FAILURE_OUTPUT_MAX_CHARS: usize = 16 * 1024;

pub(super) fn scrub_log_text(raw: &str) -> String {
    const MAX_CHARS: usize = 320;
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out = String::new();
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
    if raw.chars().count() <= RCLONE_FAILURE_OUTPUT_MAX_CHARS {
        return raw;
    }
    let mut truncated = raw
        .chars()
        .take(RCLONE_FAILURE_OUTPUT_MAX_CHARS)
        .collect::<String>();
    truncated.push_str("… [truncated]");
    truncated
}

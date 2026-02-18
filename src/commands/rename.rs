//! Rename preview helpers used by the advanced rename UI.

use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedRenamePayload {
    pub regex: String,
    pub replacement: String,
    pub prefix: String,
    pub suffix: String,
    pub case_sensitive: bool,
    pub sequence_mode: SequenceMode,
    pub sequence_placement: SequencePlacement,
    pub sequence_start: i64,
    pub sequence_step: i64,
    pub sequence_pad: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SequenceMode {
    None,
    Numeric,
    Alpha,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SequencePlacement {
    Start,
    End,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewEntry {
    #[allow(dead_code)]
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenamePreviewRow {
    pub original: String,
    pub next: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewResult {
    pub rows: Vec<RenamePreviewRow>,
    pub error: Option<String>,
}

fn split_ext(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.') else {
        return (name, "");
    };
    if dot == 0 || dot + 1 == name.len() {
        return (name, "");
    }
    name.split_at(dot)
}

fn to_alpha(n: i64) -> String {
    if n < 0 {
        return String::new();
    }
    let mut num = n;
    let mut out = String::new();
    loop {
        let rem = (num % 26) as u8;
        out.insert(0, (b'A' + rem) as char);
        num = (num / 26) - 1;
        if num < 0 {
            break;
        }
    }
    out
}

fn left_pad_zeros(value: &str, width: usize) -> String {
    if width == 0 || value.len() >= width {
        return value.to_string();
    }
    format!("{}{}", "0".repeat(width - value.len()), value)
}

fn format_sequence(index: usize, payload: &AdvancedRenamePayload) -> String {
    let value = payload
        .sequence_start
        .saturating_add((index as i64).saturating_mul(payload.sequence_step));
    match payload.sequence_mode {
        SequenceMode::None => String::new(),
        SequenceMode::Numeric => {
            let pad = payload.sequence_pad.clamp(0, 64) as usize;
            left_pad_zeros(&value.to_string(), pad)
        }
        SequenceMode::Alpha => {
            let alpha_index = (value - 1).max(0);
            to_alpha(alpha_index)
        }
    }
}

pub(crate) fn compute_advanced_rename_preview(
    entries: &[RenamePreviewEntry],
    payload: &AdvancedRenamePayload,
) -> RenamePreviewResult {
    let trimmed = payload.regex.trim();
    let mut error: Option<String> = None;
    let pattern = if trimmed.is_empty() {
        None
    } else {
        match RegexBuilder::new(trimmed)
            .case_insensitive(!payload.case_sensitive)
            .build()
        {
            Ok(regex) => Some(regex),
            Err(err) => {
                error = Some(format!("Invalid regex: {err}"));
                None
            }
        }
    };

    let rows = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let source = entry.name.as_str();
            let seq = format_sequence(idx, payload);

            let mut next = if let Some(regex) = pattern.as_ref() {
                regex
                    .replace_all(source, payload.replacement.as_str())
                    .to_string()
            } else if trimmed.is_empty() && !payload.replacement.is_empty() {
                payload.replacement.clone()
            } else {
                source.to_string()
            };

            if !payload.prefix.is_empty() || !payload.suffix.is_empty() {
                let (stem, ext) = split_ext(&next);
                next = format!("{}{}{}{}", payload.prefix, stem, payload.suffix, ext);
            }

            if payload.sequence_mode != SequenceMode::None {
                if next.contains("$n") {
                    next = next.replace("$n", &seq);
                } else {
                    let (stem, ext) = split_ext(&next);
                    next = match payload.sequence_placement {
                        SequencePlacement::Start => format!("{seq}{stem}{ext}"),
                        SequencePlacement::End => format!("{stem}{seq}{ext}"),
                    };
                }
            }

            RenamePreviewRow {
                original: source.to_string(),
                next,
            }
        })
        .collect();

    RenamePreviewResult { rows, error }
}

#[tauri::command]
pub fn preview_rename_entries(
    entries: Vec<RenamePreviewEntry>,
    payload: AdvancedRenamePayload,
) -> RenamePreviewResult {
    compute_advanced_rename_preview(&entries, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<RenamePreviewEntry> {
        vec![
            RenamePreviewEntry {
                path: "/tmp/a.txt".into(),
                name: "a.txt".into(),
            },
            RenamePreviewEntry {
                path: "/tmp/b.txt".into(),
                name: "b.txt".into(),
            },
        ]
    }

    #[test]
    fn preview_applies_regex_prefix_suffix_and_sequence() {
        let payload = AdvancedRenamePayload {
            regex: "(a|b)".into(),
            replacement: "renamed-$1".into(),
            prefix: "pre-".into(),
            suffix: "-suf".into(),
            case_sensitive: true,
            sequence_mode: SequenceMode::Numeric,
            sequence_placement: SequencePlacement::End,
            sequence_start: 1,
            sequence_step: 1,
            sequence_pad: 2,
        };
        let result = compute_advanced_rename_preview(&sample_entries(), &payload);
        assert!(result.error.is_none());
        assert_eq!(result.rows[0].next, "pre-renamed-a-suf01.txt");
        assert_eq!(result.rows[1].next, "pre-renamed-b-suf02.txt");
    }

    #[test]
    fn preview_reports_invalid_regex_without_failing_command() {
        let payload = AdvancedRenamePayload {
            regex: "[".into(),
            replacement: "x".into(),
            prefix: String::new(),
            suffix: String::new(),
            case_sensitive: true,
            sequence_mode: SequenceMode::None,
            sequence_placement: SequencePlacement::End,
            sequence_start: 1,
            sequence_step: 1,
            sequence_pad: 2,
        };
        let result = compute_advanced_rename_preview(&sample_entries(), &payload);
        assert!(result.error.is_some());
        assert_eq!(result.rows[0].next, "a.txt");
        assert_eq!(result.rows[1].next, "b.txt");
    }
}

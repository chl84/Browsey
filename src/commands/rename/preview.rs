use super::{
    AdvancedRenamePayload, RenamePreviewEntry, RenamePreviewResult, RenamePreviewRow, SequenceMode,
    SequencePlacement,
};
use regex::RegexBuilder;

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
            let (source_stem, source_ext) = if payload.keep_extension {
                split_ext(source)
            } else {
                (source, "")
            };
            let working_source = if payload.keep_extension {
                source_stem
            } else {
                source
            };
            let seq = format_sequence(idx, payload);

            let mut next = if let Some(regex) = pattern.as_ref() {
                regex
                    .replace_all(working_source, payload.replacement.as_str())
                    .to_string()
            } else if trimmed.is_empty() && !payload.replacement.is_empty() {
                payload.replacement.clone()
            } else {
                working_source.to_string()
            };

            if payload.keep_extension {
                next.push_str(source_ext);
            }

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

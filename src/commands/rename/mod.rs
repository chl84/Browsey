//! Rename commands and preview helpers used by the advanced rename UI.

use super::path_guard::{ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink};
use crate::{
    errors::api_error::ApiResult,
    fs_utils::sanitize_path_nofollow,
    undo::{
        assert_path_snapshot, is_destination_exists_error, move_with_fallback, run_actions,
        snapshot_existing_path, Action, Direction, UndoState,
    },
};
mod error;
use error::map_api_result;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntryRequest {
    pub path: String,
    pub new_name: String,
}

fn build_rename_target(from: &Path, new_name: &str) -> Result<PathBuf, String> {
    if new_name.trim().is_empty() {
        return Err("New name cannot be empty".into());
    }
    let parent = from
        .parent()
        .ok_or_else(|| "Cannot rename root".to_string())?;
    Ok(parent.join(new_name.trim()))
}

fn prepare_rename_pair(path: &str, new_name: &str) -> Result<(PathBuf, PathBuf), String> {
    let from = sanitize_path_nofollow(path, true)?;
    let to = build_rename_target(&from, new_name)?;
    Ok((from, to))
}

fn apply_rename(from: &Path, to: &Path) -> Result<(), String> {
    ensure_existing_path_nonsymlink(from)?;
    let from_snapshot = snapshot_existing_path(from)?;
    if let Some(parent) = to.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
        let parent_snapshot = snapshot_existing_path(parent)?;
        assert_path_snapshot(parent, &parent_snapshot)?;
    } else {
        return Err("Invalid destination path".into());
    }
    assert_path_snapshot(from, &from_snapshot)?;
    match move_with_fallback(from, to) {
        Ok(_) => Ok(()),
        Err(e) if is_destination_exists_error(&e) => {
            Err("A file or directory with that name already exists".into())
        }
        Err(e) => Err(format!("Failed to rename: {e}")),
    }
}

pub(crate) fn rename_entry_impl(
    path: &str,
    new_name: &str,
    state: &UndoState,
) -> Result<String, String> {
    let (from, to) = prepare_rename_pair(path, new_name)?;
    apply_rename(&from, &to)?;
    let _ = state.record_applied(Action::Rename {
        from: from.clone(),
        to: to.clone(),
    });
    Ok(to.to_string_lossy().to_string())
}

pub(crate) fn rename_entries_impl(
    entries: Vec<RenameEntryRequest>,
    undo: &UndoState,
) -> Result<Vec<String>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(entries.len());
    let mut seen_sources: HashSet<PathBuf> = HashSet::with_capacity(entries.len());
    let mut seen_targets: HashSet<PathBuf> = HashSet::with_capacity(entries.len());

    for (idx, entry) in entries.into_iter().enumerate() {
        let (from, to) = prepare_rename_pair(entry.path.as_str(), entry.new_name.as_str())?;

        if !seen_sources.insert(from.clone()) {
            return Err(format!(
                "Duplicate source path in request (item {})",
                idx + 1
            ));
        }
        if !seen_targets.insert(to.clone()) {
            return Err(format!(
                "Duplicate target name in request (item {})",
                idx + 1
            ));
        }

        pairs.push((from, to));
    }

    let mut performed: Vec<Action> = Vec::new();
    let mut renamed_paths: Vec<String> = Vec::with_capacity(pairs.len());

    for (from, to) in pairs {
        if from == to {
            continue;
        }
        if let Err(err) = apply_rename(&from, &to) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                    return Err(format!("{}; rollback also failed: {}", err, rb_err));
                }
            }
            return Err(err);
        }

        renamed_paths.push(to.to_string_lossy().to_string());
        performed.push(Action::Rename {
            from: from.clone(),
            to: to.clone(),
        });
    }

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
    }

    Ok(renamed_paths)
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
pub fn rename_entry(
    path: String,
    new_name: String,
    state: tauri::State<UndoState>,
) -> ApiResult<String> {
    map_api_result(rename_entry_impl(
        path.as_str(),
        new_name.as_str(),
        state.inner(),
    ))
}

#[tauri::command]
pub fn rename_entries(
    entries: Vec<RenameEntryRequest>,
    undo: tauri::State<UndoState>,
) -> ApiResult<Vec<String>> {
    map_api_result(rename_entries_impl(entries, undo.inner()))
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
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

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

    fn uniq_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-rename-test-{label}-{ts}"))
    }

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap();
        file.write_all(content).unwrap();
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

    #[test]
    fn rename_entry_impl_supports_undo_redo() {
        let dir = uniq_path("single");
        let _ = fs::create_dir_all(&dir);
        let from = dir.join("before.txt");
        write_file(&from, b"data");
        let state = UndoState::default();

        let renamed = rename_entry_impl(from.to_string_lossy().as_ref(), "after.txt", &state)
            .expect("rename should succeed");
        let to = PathBuf::from(renamed);

        assert!(!from.exists());
        assert!(to.exists());

        state.undo().expect("undo should succeed");
        assert!(from.exists());
        assert!(!to.exists());

        state.redo().expect("redo should succeed");
        assert!(!from.exists());
        assert!(to.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_entries_impl_rolls_back_when_later_item_fails() {
        let dir = uniq_path("batch-rollback");
        let _ = fs::create_dir_all(&dir);
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        let existing = dir.join("existing.txt");
        write_file(&a, b"a");
        write_file(&b, b"b");
        write_file(&existing, b"existing");
        let state = UndoState::default();

        let err = rename_entries_impl(
            vec![
                RenameEntryRequest {
                    path: a.to_string_lossy().to_string(),
                    new_name: "a-renamed.txt".into(),
                },
                RenameEntryRequest {
                    path: b.to_string_lossy().to_string(),
                    new_name: "existing.txt".into(),
                },
            ],
            &state,
        )
        .expect_err("batch should fail on second rename");

        assert!(err.contains("already exists"), "unexpected error: {err}");
        assert!(a.exists(), "first rename should be rolled back");
        assert!(b.exists(), "second source should remain");
        assert!(!dir.join("a-renamed.txt").exists());
        assert!(existing.exists());
        assert!(
            state.undo().is_err(),
            "failed batch should not record undo state"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_entries_impl_rejects_duplicate_source_paths() {
        let dir = uniq_path("duplicate-source");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("same.txt");
        write_file(&path, b"same");
        let state = UndoState::default();

        let err = rename_entries_impl(
            vec![
                RenameEntryRequest {
                    path: path.to_string_lossy().to_string(),
                    new_name: "first.txt".into(),
                },
                RenameEntryRequest {
                    path: path.to_string_lossy().to_string(),
                    new_name: "second.txt".into(),
                },
            ],
            &state,
        )
        .expect_err("duplicate source should fail");

        assert!(
            err.contains("Duplicate source path"),
            "unexpected error: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

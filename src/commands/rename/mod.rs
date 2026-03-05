//! Rename commands and preview helpers used by the advanced rename UI.

use crate::{errors::api_error::ApiResult, undo::UndoState};
mod error;
mod ops;
mod preview;
use error::map_api_result;
use ops::{rename_entries_impl, rename_entry_impl};
use preview::compute_advanced_rename_preview;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedRenamePayload {
    pub regex: String,
    pub replacement: String,
    pub prefix: String,
    pub suffix: String,
    pub case_sensitive: bool,
    #[serde(default = "default_keep_extension")]
    pub keep_extension: bool,
    pub sequence_mode: SequenceMode,
    pub sequence_placement: SequencePlacement,
    pub sequence_start: i64,
    pub sequence_step: i64,
    pub sequence_pad: i64,
}

fn default_keep_extension() -> bool {
    true
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
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};
    #[cfg(unix)]
    use std::{fs::Permissions, os::unix::fs::PermissionsExt};

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
            keep_extension: true,
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
            keep_extension: true,
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
    fn preview_can_modify_extension_when_keep_extension_is_disabled() {
        let payload = AdvancedRenamePayload {
            regex: "\\.txt$".into(),
            replacement: ".bak".into(),
            prefix: String::new(),
            suffix: String::new(),
            case_sensitive: true,
            keep_extension: false,
            sequence_mode: SequenceMode::None,
            sequence_placement: SequencePlacement::End,
            sequence_start: 1,
            sequence_step: 1,
            sequence_pad: 2,
        };
        let result = compute_advanced_rename_preview(&sample_entries(), &payload);
        assert!(result.error.is_none());
        assert_eq!(result.rows[0].next, "a.bak");
        assert_eq!(result.rows[1].next, "b.bak");
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

        assert!(
            err.to_string().contains("already exists"),
            "unexpected error: {err}"
        );
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
            err.to_string().contains("Duplicate source path"),
            "unexpected error: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_entry_impl_rejects_existing_target_without_overwrite() {
        let dir = uniq_path("single-target-exists");
        let _ = fs::create_dir_all(&dir);
        let from = dir.join("from.txt");
        let existing = dir.join("existing.txt");
        write_file(&from, b"from");
        write_file(&existing, b"existing");
        let state = UndoState::default();

        let err = rename_entry_impl(from.to_string_lossy().as_ref(), "existing.txt", &state)
            .expect_err("rename should fail when target exists");

        assert!(
            err.to_string().contains("already exists"),
            "unexpected error: {err}"
        );
        assert!(
            from.exists(),
            "source should remain when rename is rejected"
        );
        assert!(existing.exists(), "existing target should remain untouched");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_entry_impl_fails_when_source_is_missing() {
        let dir = uniq_path("single-missing-source");
        let _ = fs::create_dir_all(&dir);
        let missing = dir.join("missing.txt");
        let state = UndoState::default();

        let err = rename_entry_impl(missing.to_string_lossy().as_ref(), "renamed.txt", &state)
            .expect_err("rename should fail for missing source");

        assert!(
            err.to_string().to_lowercase().contains("no such file")
                || err.to_string().to_lowercase().contains("not found"),
            "unexpected error: {err}"
        );
        assert!(
            !dir.join("renamed.txt").exists(),
            "destination should not be created on failure"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_entry_impl_fails_when_parent_directory_is_read_only() {
        let dir = uniq_path("single-read-only-parent");
        let _ = fs::create_dir_all(&dir);
        let from = dir.join("before.txt");
        write_file(&from, b"data");
        let state = UndoState::default();

        fs::set_permissions(&dir, Permissions::from_mode(0o555)).expect("set read-only dir");
        let err = rename_entry_impl(from.to_string_lossy().as_ref(), "after.txt", &state)
            .expect_err("rename should fail in read-only directory");

        assert!(
            err.to_string().to_lowercase().contains("permission")
                || err.to_string().to_lowercase().contains("denied"),
            "unexpected error: {err}"
        );
        assert!(from.exists(), "source should remain unchanged");
        assert!(
            !dir.join("after.txt").exists(),
            "target should not be created"
        );

        fs::set_permissions(&dir, Permissions::from_mode(0o755)).expect("restore permissions");
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_entry_impl_rejects_symlink_source_no_follow() {
        let dir = uniq_path("single-symlink-no-follow");
        let _ = fs::create_dir_all(&dir);
        let real = dir.join("real.txt");
        write_file(&real, b"data");
        let link = dir.join("link.txt");
        symlink(&real, &link).expect("create symlink");
        let state = UndoState::default();

        let err = rename_entry_impl(link.to_string_lossy().as_ref(), "renamed.txt", &state)
            .expect_err("rename should reject symlink source");

        assert!(
            err.to_string().to_lowercase().contains("symlink"),
            "unexpected error: {err}"
        );
        assert!(link.exists(), "symlink should remain unchanged");
        assert!(real.exists(), "real file should remain unchanged");
        assert!(
            !dir.join("renamed.txt").exists(),
            "target should not be created"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

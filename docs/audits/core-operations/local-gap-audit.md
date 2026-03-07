# Core Operations Local Test Gap Audit

Created: 2026-03-02
Scope: Local destructive-operation hardening (Step 3 in
`docs/todo-archive/TODO_CORE_OPERATIONS_HARDENING.md`)
Matrix reference: `docs/operations/core-operations/matrix.md` (`LCM`, `LRN`, `LTD`)

## Evidence Reviewed

- `src/clipboard/tests.rs`
- `src/commands/rename/mod.rs` (`#[cfg(test)] mod tests`)
- `src/commands/fs/trash/tests.rs`
- `src/commands/fs/delete_ops.rs` (`#[cfg(test)] mod tests`)
- `src/commands/fs/trash/mod.rs` + `src/commands/fs/trash/tests.rs` (restore/purge core tests)

## Coverage vs Matrix (Local Families)

| Scenario ID | Current automated coverage | Evidence | Gap summary |
|---|---|---|---|
| `CO-LCM-001` | Partial | `copy_file_best_effort_does_not_overwrite_existing_target`, `copy_file_best_effort_fails_when_source_is_missing`, `copy_file_best_effort_fails_when_destination_dir_is_read_only`, `copy_file_best_effort_cancelled_before_transfer_removes_destination`, `paste_clipboard_copy_rolls_back_successful_items_when_later_source_fails`, `paste_clipboard_copy_cancelled_after_first_item_rolls_back_created_targets` | Missing broader multi-item hostile-condition coverage beyond the added rollback-on-later-failure and mid-batch-cancel paths. |
| `CO-LCM-002` | Partial | `merge_copy_can_undo_without_touching_existing`, `copy_entry_rejects_symlink_source_no_follow` | Missing recursive multi-item partial-failure summary assertions. |
| `CO-LCM-003` | Partial | `move_entry_does_not_overwrite_existing_target`, `move_entry_fails_when_source_is_missing`, `move_entry_keeps_source_when_destination_parent_disappears`, `move_entry_fails_when_destination_dir_is_read_only_and_keeps_source`, `paste_clipboard_cut_rolls_back_successful_items_when_later_source_fails` | Missing explicit cancellation coverage and broader per-item partial-summary coverage beyond the added later-failure rollback path. |
| `CO-LCM-004` | Partial | `merge_cut_undo_restores_source_and_target` | Missing partial directory move failure assertions and progress/cancel behavior. |
| `CO-LRN-001` | Partial | `rename_entry_impl_supports_undo_redo`, `rename_entry_impl_fails_when_parent_directory_is_read_only`, `rename_entry_impl_rejects_symlink_source_no_follow` | Missing cancel-from-UI proxy coverage. |
| `CO-LRN-002` | Partial | `rename_entry_impl_supports_undo_redo` | Missing folder-specific failure/permission variants. |
| `CO-LRN-003` | Partial | `rename_entries_impl_rolls_back_when_later_item_fails`, `rename_entry_impl_rejects_existing_target_without_overwrite`, `rename_entry_impl_fails_when_source_is_missing` | Missing cancel-from-UI proxy cases and permission-denied/read-only variants. |
| `CO-LTD-001` | Partial | `move_single_to_trash_uses_backend_and_rewrites_original_path` | Missing cancellation and permission-denied paths. |
| `CO-LTD-002` | Partial | `move_to_trash_many_rolls_back_previous_on_later_failure` | Missing recursive directory trash failure matrix (disappearing source/destination). |
| `CO-LTD-003` | Partial | `restore_with_ops_restores_selected_ids_and_emits_change`, `restore_with_ops_rejects_empty_selection_after_filtering`, `restore_with_ops_conflict_failure_does_not_emit_change`, `restore_with_ops_list_failure_does_not_emit_change` | Missing broader system-backend hostile-condition coverage beyond the added conflict/list-failure paths. |
| `CO-LTD-004` | Partial | `purge_with_ops_purges_selected_ids_and_emits_change`, `purge_with_ops_failure_does_not_emit_change`, `purge_with_ops_list_failure_does_not_emit_change` | Missing broader system-backend purge hostile-condition coverage beyond the added list-failure path. |
| `CO-LTD-005` | Partial | `delete_with_backup_can_undo_single_file`, `delete_with_backup_can_undo_directory_tree`, `delete_entries_with_hooks_rolls_back_when_later_item_fails`, `delete_entries_with_hooks_cancellation_rolls_back_completed_items`, `delete_entries_with_hooks_records_undo_for_successful_batch`, `delete_entries_with_hooks_permission_denied_keeps_sources_and_reports_error` | Still missing broader hostile-condition coverage beyond the added permission-denied variant. |

## Priority Gaps to Close Next

1. Expand multi-item local copy/move hostile-condition coverage beyond the added later-source-failure and mid-batch-cancel paths.
2. Add real system-backend restore/purge hostile-condition coverage beyond the current fake-ops list/conflict/failure paths.
3. Add broader local rename cancel/proxy coverage.

## Notes

- Current local coverage has good rollback-oriented building blocks, but is
  skewed toward happy-path + collision checks.
- Highest trust risk in local scope right now is uncovered destructive and
  recovery-sensitive behavior (`restore`, `purge`, and permanent delete).

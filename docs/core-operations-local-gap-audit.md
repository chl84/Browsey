# Core Operations Local Test Gap Audit

Created: 2026-03-02
Scope: Local destructive-operation hardening (Step 3 in
`docs/TODO_CORE_OPERATIONS_HARDENING.md`)
Matrix reference: `docs/core-operations-matrix.md` (`LCM`, `LRN`, `LTD`)

## Evidence Reviewed

- `src/clipboard/tests.rs`
- `src/commands/rename/mod.rs` (`#[cfg(test)] mod tests`)
- `src/commands/fs/trash/tests.rs`
- `src/commands/fs/delete_ops.rs` (`#[cfg(test)] mod tests`)
- `src/commands/fs/trash/mod.rs` + `src/commands/fs/trash/tests.rs` (restore/purge core tests)

## Coverage vs Matrix (Local Families)

| Scenario ID | Current automated coverage | Evidence | Gap summary |
|---|---|---|---|
| `CO-LCM-001` | Partial | `copy_file_best_effort_does_not_overwrite_existing_target`, `copy_file_best_effort_fails_when_source_is_missing` | Missing cancel-path and multi-item partial-failure assertions. |
| `CO-LCM-002` | Partial | `merge_copy_can_undo_without_touching_existing` | Missing permission-denied/disappearing-path cases and user-facing partial summary checks. |
| `CO-LCM-003` | Partial | `move_entry_does_not_overwrite_existing_target`, `move_entry_fails_when_source_is_missing`, `move_entry_keeps_source_when_destination_parent_disappears` | Missing cancellation coverage and explicit per-item partial summary assertions. |
| `CO-LCM-004` | Partial | `merge_cut_undo_restores_source_and_target` | Missing partial directory move failure assertions and progress/cancel behavior. |
| `CO-LRN-001` | Partial | `rename_entry_impl_supports_undo_redo` | Missing permission-denied/read-only variants. |
| `CO-LRN-002` | Partial | `rename_entry_impl_supports_undo_redo` | Missing folder-specific failure/permission variants. |
| `CO-LRN-003` | Partial | `rename_entries_impl_rolls_back_when_later_item_fails`, `rename_entry_impl_rejects_existing_target_without_overwrite`, `rename_entry_impl_fails_when_source_is_missing` | Missing cancel-from-UI proxy cases and permission-denied/read-only variants. |
| `CO-LTD-001` | Partial | `move_single_to_trash_uses_backend_and_rewrites_original_path` | Missing cancellation and permission-denied paths. |
| `CO-LTD-002` | Partial | `move_to_trash_many_rolls_back_previous_on_later_failure` | Missing recursive directory trash failure matrix (disappearing source/destination). |
| `CO-LTD-003` | Partial | `restore_with_ops_restores_selected_ids_and_emits_change`, `restore_with_ops_rejects_empty_selection_after_filtering` | Missing integration coverage of system-backend failure modes and restore-conflict paths. |
| `CO-LTD-004` | Partial | `purge_with_ops_purges_selected_ids_and_emits_change`, `purge_with_ops_failure_does_not_emit_change` | Missing integration coverage of system-backend listing/purge errors via real trash backend. |
| `CO-LTD-005` | Partial | `delete_with_backup_can_undo_single_file`, `delete_with_backup_can_undo_directory_tree` | Missing `delete_entries_blocking` cancellation/partial-failure coverage and progress-path assertions. |

## Priority Gaps to Close Next

1. Add direct unit/integration tests for `restore_trash_items_impl` and
   `purge_trash_items_impl` (currently uncovered matrix scenarios).
2. Add tests around `delete_entries_blocking` cancellation and rollback behavior
   in `src/commands/fs/delete_ops.rs`.
3. Add fault-oriented local clipboard tests for disappearing source/destination
   and permission-denied paths.
4. Add rename permission/read-only tests for file and folder targets.

## Notes

- Current local coverage has good rollback-oriented building blocks, but is
  skewed toward happy-path + collision checks.
- Highest trust risk in local scope right now is uncovered destructive and
  recovery-sensitive behavior (`restore`, `purge`, and permanent delete).

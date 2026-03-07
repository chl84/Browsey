# Core Operations Mixed Transfer Gap Audit

Created: 2026-03-02
Scope: Mixed local<->cloud transfer hardening (Step 4 in
`docs/todo-archive/TODO_CORE_OPERATIONS_HARDENING.md`)
Matrix reference: `docs/operations/core-operations/matrix.md` (`MTC`)

## Evidence Reviewed

- `src/commands/transfer/execute.rs` (`#[cfg(test)] mod tests`)
- `src/commands/transfer/preview.rs` (`#[cfg(test)] mod tests`)
- `src/commands/transfer/route.rs` (`#[cfg(test)] mod tests`)
- `src/commands/cloud/providers/rclone/tests.rs` (fake-`rclone` integration suite)
- `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts`

## Coverage vs Matrix (Mixed Family)

| Scenario ID | Current automated coverage | Evidence | Gap summary |
|---|---|---|---|
| `CO-MTC-001` | Partial | `mixed_execute_local_to_cloud_file_copy_and_move_via_fake_rclone`, `executes local-to-cloud copy via explicit mixed target command`, `register_mixed_cancel_progress_event_sets_token_on_cancel`, `mixed_execute_local_to_cloud_copy_cancels_during_second_active_transfer`, `mixed_execute_local_to_cloud_progress_batch_copy_cancels_during_second_active_transfer` | Active mid-batch cancellation is now covered for both CLI-loop and progress-aware copy paths; more hostile-condition breadth is still needed elsewhere in the family. |
| `CO-MTC-002` | Partial | `mixed_execute_cloud_to_local_file_copy_and_move_via_fake_rclone`, `register_mixed_cancel_progress_event_sets_token_on_cancel`, `mixed_execute_cloud_to_local_copy_cancels_during_second_active_transfer`, `mixed_execute_cloud_to_local_progress_batch_copy_cancels_during_second_active_transfer` | Active mid-batch cancellation is now covered for both CLI-loop and progress-aware copy paths; more hostile-condition breadth is still needed elsewhere in the family. |
| `CO-MTC-003` | Partial | `mixed_execute_local_to_cloud_file_copy_and_move_via_fake_rclone`, `mixed_execute_local_to_cloud_partial_move_removes_first_source`, `mixed_execute_local_to_cloud_move_cancels_during_second_active_transfer`, `mixed_execute_local_to_cloud_progress_batch_move_cancels_during_second_active_transfer`, `refreshes cloud view after mixed local-to-cloud failure to reconcile partial writes` | Active mid-batch cancellation is now covered for move in both the CLI-loop and progress-aware file-loop paths; broader hostile-condition breadth still remains open. |
| `CO-MTC-004` | Partial | `mixed_execute_cloud_to_local_file_copy_and_move_via_fake_rclone`, `executes cloud-to-local move via explicit mixed target command and clears cut clipboard state`, `register_mixed_cancel_progress_event_sets_token_on_cancel`, `mixed_execute_cloud_to_local_move_cancels_during_second_active_transfer`, `mixed_execute_cloud_to_local_progress_batch_move_cancels_during_second_active_transfer` | Active mid-batch cancellation is now covered for move in both the CLI-loop and progress-aware file-loop paths; broader hostile-condition breadth still remains open. |
| `CO-MTC-005` | Partial | `mixed_execute_local_to_cloud_directory_copy_and_move_via_fake_rclone`, `mixed_execute_cloud_to_local_directory_copy_and_move_via_fake_rclone`, `mixed_execute_local_to_cloud_partial_directory_move_invalidates_cache_and_keeps_partial_state`, `attempts local refresh after mixed cloud-to-local failure to reconcile partial writes` | Execute-phase progress cancellation and broader partial-summary coverage still need direct assertions for active directory transfer loops. |
| `CO-MTC-006` | Partial | `mixed_preview_local_to_cloud_matches_onedrive_case_insensitive_and_preserves_kind`, `mixed_preview_cloud_to_local_reports_file_and_dir_conflicts`, `resolves mixed local-to-cloud rename-on-conflict by retrying explicit target candidates`, `resolves mixed cloud-to-local rename-on-conflict for move by retrying target candidates`, `refreshes cloud view after mixed local-to-cloud failure to reconcile partial writes`, `attempts local refresh after mixed cloud-to-local failure to reconcile partial writes` | Preview/execute consistency is stronger, but execute-phase progress cancellation and broader hostile-condition assertions still need direct coverage for the remaining active transfer families. |

## Notable Hardening Added

- Partial local->cloud completion now invalidates cloud cache even on error:
  `mixed_execute_local_to_cloud_partial_copy_keeps_source_and_invalidates_cache`.
- Copy vs move cleanup semantics now asserted under partial failure:
  `mixed_execute_local_to_cloud_partial_copy_keeps_source_and_invalidates_cache`
  and `mixed_execute_local_to_cloud_partial_move_removes_first_source`.
- Consistent transfer error-code mapping for non-zero rclone failures:
  `maps_rclone_nonzero_errors_to_consistent_transfer_codes`.
- Hostile-condition directory batch behavior is covered for partial move state:
  `mixed_execute_local_to_cloud_partial_directory_move_invalidates_cache_and_keeps_partial_state`.
- Progress-event cancellation registration behavior is covered in mixed transfer:
  `register_mixed_cancel_returns_none_without_progress_event` and
  `register_mixed_cancel_progress_event_sets_token_on_cancel`.
- Active mid-batch cancellation is now covered for mixed copy loops in both
  directions:
  `mixed_execute_local_to_cloud_copy_cancels_during_second_active_transfer`
  and `mixed_execute_cloud_to_local_copy_cancels_during_second_active_transfer`.
- Active mid-batch cancellation is now also covered for mixed move loops in
  both directions:
  `mixed_execute_local_to_cloud_move_cancels_during_second_active_transfer`
  and `mixed_execute_cloud_to_local_move_cancels_during_second_active_transfer`.
- Progress-aware mixed copy loops now also have direct active-cancel coverage in
  both directions:
  `mixed_execute_local_to_cloud_progress_batch_copy_cancels_during_second_active_transfer`
  and `mixed_execute_cloud_to_local_progress_batch_copy_cancels_during_second_active_transfer`.
- Progress-aware mixed move loops now also have direct active-cancel coverage in
  both directions:
  `mixed_execute_local_to_cloud_progress_batch_move_cancels_during_second_active_transfer`
  and `mixed_execute_cloud_to_local_progress_batch_move_cancels_during_second_active_transfer`.
- Frontend operation-state integrity now asserts refresh reconciliation after
  mixed partial failures:
  `refreshes cloud view after mixed local-to-cloud failure to reconcile partial writes`
  and `attempts local refresh after mixed cloud-to-local failure to reconcile partial writes`.

## Priority Gaps to Close Next

1. Extend mixed hostile-condition coverage beyond cancellation into broader
   directory progress-path and partial-summary assertions where the matrix still says
   `Partial`.

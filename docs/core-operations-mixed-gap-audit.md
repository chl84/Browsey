# Core Operations Mixed Transfer Gap Audit

Created: 2026-03-02
Scope: Mixed local<->cloud transfer hardening (Step 4 in
`docs/TODO_CORE_OPERATIONS_HARDENING.md`)
Matrix reference: `docs/core-operations-matrix.md` (`MTC`)

## Evidence Reviewed

- `src/commands/transfer/execute.rs` (`#[cfg(test)] mod tests`)
- `src/commands/transfer/preview.rs` (`#[cfg(test)] mod tests`)
- `src/commands/transfer/route.rs` (`#[cfg(test)] mod tests`)
- `src/commands/cloud/providers/rclone/tests.rs` (fake-`rclone` integration suite)
- `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts`

## Coverage vs Matrix (Mixed Family)

| Scenario ID | Current automated coverage | Evidence | Gap summary |
|---|---|---|---|
| `CO-MTC-001` | Partial | `mixed_execute_local_to_cloud_file_copy_and_move_via_fake_rclone`, `executes local-to-cloud copy via explicit mixed target command` | Progress-event cancellation path still needs direct coverage. |
| `CO-MTC-002` | Partial | `mixed_execute_cloud_to_local_file_copy_and_move_via_fake_rclone` | Progress-event cancellation path still needs direct coverage. |
| `CO-MTC-003` | Partial | `mixed_execute_local_to_cloud_file_copy_and_move_via_fake_rclone`, `mixed_execute_local_to_cloud_partial_move_removes_first_source` | Mid-batch provider failure recovery is partial-state by design; needs clearer assertion coverage for UI follow-up. |
| `CO-MTC-004` | Partial | `mixed_execute_cloud_to_local_file_copy_and_move_via_fake_rclone`, `executes cloud-to-local move via explicit mixed target command and clears cut clipboard state` | Progress-event cancellation path still needs direct coverage. |
| `CO-MTC-005` | Partial | `mixed_execute_local_to_cloud_directory_copy_and_move_via_fake_rclone`, `mixed_execute_cloud_to_local_directory_copy_and_move_via_fake_rclone` | Add one hostile-condition directory case where a later item fails after earlier success in the same batch. |
| `CO-MTC-006` | Partial | `mixed_preview_local_to_cloud_matches_onedrive_case_insensitive_and_preserves_kind`, `mixed_preview_cloud_to_local_reports_file_and_dir_conflicts`, `resolves mixed local-to-cloud rename-on-conflict by retrying explicit target candidates`, `resolves mixed cloud-to-local rename-on-conflict for move by retrying target candidates` | Execute-phase progress-aware cancellation assertion still missing. |

## Notable Hardening Added

- Partial local->cloud completion now invalidates cloud cache even on error:
  `mixed_execute_local_to_cloud_partial_copy_keeps_source_and_invalidates_cache`.
- Copy vs move cleanup semantics now asserted under partial failure:
  `mixed_execute_local_to_cloud_partial_copy_keeps_source_and_invalidates_cache`
  and `mixed_execute_local_to_cloud_partial_move_removes_first_source`.
- Consistent transfer error-code mapping for non-zero rclone failures:
  `maps_rclone_nonzero_errors_to_consistent_transfer_codes`.

## Priority Gaps to Close Next

1. Add direct mixed-transfer cancellation coverage with active progress-event
   context (not only early-cancel token checks).
2. Add one directory batch hostile-condition test where later item fails after
   earlier success, then assert expected partial-state summary behavior.
3. Add one frontend regression assertion for post-error refresh/invalidation
   behavior after mixed partial completion.

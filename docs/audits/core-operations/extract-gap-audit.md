# Core Operations Extract Gap Audit

Created: 2026-03-02
Scope: Archive extraction hardening (Step 5 in
`docs/todo-archive/TODO_CORE_OPERATIONS_HARDENING.md`)
Matrix reference: `docs/operations/core-operations/matrix.md` (`EXT`)

## Evidence Reviewed

- `src/commands/decompress/mod.rs`
- `src/commands/decompress/util.rs`
- `src/commands/decompress/zip_format.rs`
- `src/commands/decompress/tar_format.rs`
- `src/commands/decompress/seven_z_format.rs`
- `src/commands/decompress/rar_format.rs`
- `src/commands/decompress/error.rs`

## Coverage vs Matrix (Extract Family)

| Scenario ID | Current automated coverage | Evidence | Gap summary |
|---|---|---|---|
| `CO-EXT-001` | Partial | `open_unique_file_uses_suffix_when_destination_already_exists`, `do_extract_extracts_real_archive_to_directory` | A real archive-to-directory success path is now covered for `do_extract`, but broader format confidence is still incomplete. |
| `CO-EXT-002` | Partial | `open_unique_file_uses_suffix_when_destination_already_exists`, `do_extract_uses_unique_destination_when_archive_root_conflicts` | Archive-level root-destination conflict handling is now covered for `do_extract`, but broader extractor conflict-path breadth is still incomplete. |
| `CO-EXT-003` | Partial | `copy_with_progress_stops_when_cancel_is_triggered_during_large_copy`, `created_paths_drop_rolls_back_partial_outputs_unless_disarmed`, `extract_zip_rolls_back_partial_outputs_when_cancelled_mid_entry`, `do_extract_rolls_back_partial_outputs_when_cancelled_mid_entry`, `tar_archive_is_readable_and_rolls_back_when_cancelled_mid_entry` | Archive-level and direct `do_extract` cancel/rollback are now covered for zip, and tar has direct archive-level cancel/rollback coverage, but broader non-zip family confidence is still incomplete. |
| `CO-EXT-004` | Partial | `open_unique_file_reports_permission_denied_in_read_only_directory` | Permission-denied is covered at destination file-creation seam; execute-level archive write-path coverage is still missing. |
| `CO-EXT-005` | Partial | `build_batch_extract_items_continues_after_non_cancel_failure`, `build_batch_extract_items_stops_after_cancelled_error`, `build_batch_extract_items_continues_after_real_archive_failure`, `build_batch_extract_items_stops_after_real_archive_cancel`, `extract_archives_blocking_continues_after_real_archive_failure`, `extract_archives_blocking_stops_after_real_archive_cancel` | Batch decision logic and the blocking batch entrypoint are now covered with both synthetic and real archive inputs, but broader format confidence is still missing. |

## Notable Hardening Added

- Destination conflict handling at file-creation seam is now asserted:
  `open_unique_file_uses_suffix_when_destination_already_exists`.
- Mid-copy cancellation behavior is now asserted at extraction copy seam:
  `copy_with_progress_stops_when_cancel_is_triggered_during_large_copy`.
- Partial output rollback semantics are now asserted via `CreatedPaths`:
  `created_paths_drop_rolls_back_partial_outputs_unless_disarmed`.
- Zip extraction now has direct archive-level cancel + rollback coverage:
  `extract_zip_rolls_back_partial_outputs_when_cancelled_mid_entry`.
- `do_extract` now has direct real-archive success-path coverage:
  `do_extract_extracts_real_archive_to_directory`.
- `do_extract` now also has direct archive-root conflict-path coverage:
  `do_extract_uses_unique_destination_when_archive_root_conflicts`.
- `do_extract` now has direct cancel + rollback coverage with a real archive:
  `do_extract_rolls_back_partial_outputs_when_cancelled_mid_entry`.
- Tar extraction now also has direct archive-level cancel + rollback coverage:
  `tar_archive_is_readable_and_rolls_back_when_cancelled_mid_entry`.
- Permission-denied mapping is now asserted at destination creation seam:
  `open_unique_file_reports_permission_denied_in_read_only_directory`.
- Batch partial-completion/cancel branching is now asserted in batch helper:
  `build_batch_extract_items_continues_after_non_cancel_failure` and
  `build_batch_extract_items_stops_after_cancelled_error`.
- Real-archive batch continuation/stop behavior is now asserted through the same
  helper:
  `build_batch_extract_items_continues_after_real_archive_failure` and
  `build_batch_extract_items_stops_after_real_archive_cancel`.
- The blocking batch entrypoint now has direct real-archive continuation/stop
  coverage:
  `extract_archives_blocking_continues_after_real_archive_failure` and
  `extract_archives_blocking_stops_after_real_archive_cancel`.
- Non-transactional extract semantics are now explicitly documented for release
  validation:
  `docs/operations/core-operations/release-checklist.md` (`Extract Non-Transactional Notes`).
- Deterministic hostile-condition fault injection is documented and covered in
  extraction seams:
  `docs/operations/core-operations/fault-injection-notes.md`.

## Priority Gaps to Close Next

1. If the core-operations `EXT` family is later promoted beyond the current
   Linux 1.0 release claim, decide whether 7z/rar need equivalent
   command-level hostile-condition tests.
2. If future release work wants per-format parity rather than shared-stack
   confidence, decide whether additional format-specific end-to-end assertions
   are needed beyond the current zip/tar + shared `do_extract` evidence.

## Linux 1.0 Note

For the Linux 1.0 release claim, the current extract evidence is considered
strong enough even though the broader `EXT` matrix remains `Partial`:

- zip, tar, 7z, and rar all flow through the same destination-creation and
  rollback-sensitive primitives (`open_unique_file`, `CreatedPaths`, and
  shared cancel-aware write helpers)
- zip and tar already have direct archive-level cancellation + rollback
  assertions
- `do_extract` already has direct real-archive success, conflict, and cancel
  coverage

That means extra hostile-condition duplication for every archive format remains
useful future hardening work, but it is no longer treated as a Linux 1.0
release blocker for Step 3.

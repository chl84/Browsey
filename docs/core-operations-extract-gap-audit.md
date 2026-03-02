# Core Operations Extract Gap Audit

Created: 2026-03-02
Scope: Archive extraction hardening (Step 5 in
`docs/TODO_CORE_OPERATIONS_HARDENING.md`)
Matrix reference: `docs/core-operations-matrix.md` (`EXT`)

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
| `CO-EXT-001` | Partial | `open_unique_file_uses_suffix_when_destination_already_exists` | No end-to-end extractor test yet for successful archive-to-directory extraction flow. |
| `CO-EXT-002` | Partial | `open_unique_file_uses_suffix_when_destination_already_exists` | No archive-level conflict-path assertions yet for pre-existing destination trees. |
| `CO-EXT-003` | Partial | `copy_with_progress_stops_when_cancel_is_triggered_during_large_copy`, `created_paths_drop_rolls_back_partial_outputs_unless_disarmed` | Execute-level cancel flow in `do_extract` / `extract_archives_impl` still lacks direct regression coverage. |
| `CO-EXT-004` | Partial | `open_unique_file_reports_permission_denied_in_read_only_directory` | Permission-denied is covered at destination file-creation seam; execute-level archive write-path coverage is still missing. |
| `CO-EXT-005` | Partial | `build_batch_extract_items_continues_after_non_cancel_failure`, `build_batch_extract_items_stops_after_cancelled_error` | Batch decision logic is covered; integration coverage with real archives + async batch entrypoint is still missing. |

## Notable Hardening Added

- Destination conflict handling at file-creation seam is now asserted:
  `open_unique_file_uses_suffix_when_destination_already_exists`.
- Mid-copy cancellation behavior is now asserted at extraction copy seam:
  `copy_with_progress_stops_when_cancel_is_triggered_during_large_copy`.
- Partial output rollback semantics are now asserted via `CreatedPaths`:
  `created_paths_drop_rolls_back_partial_outputs_unless_disarmed`.
- Permission-denied mapping is now asserted at destination creation seam:
  `open_unique_file_reports_permission_denied_in_read_only_directory`.
- Batch partial-completion/cancel branching is now asserted in batch helper:
  `build_batch_extract_items_continues_after_non_cancel_failure` and
  `build_batch_extract_items_stops_after_cancelled_error`.

## Priority Gaps to Close Next

1. Add archive-level cancel/interruption regression that exercises `do_extract`.
2. Add integration-level multi-archive regression through `extract_archives_impl`
   with real archive fixtures.

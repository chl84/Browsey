# Linux Stabilization Regression Audit

Created: 2026-03-07
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 7 (`Add regression tests for every stabilization bug that gets fixed`)

## Purpose

Track the concrete stabilization bugs fixed during the Linux production-ready
branch and verify that each one gained focused regression coverage rather than
being fixed only incidentally.

This is intentionally narrower than "all bugs ever found in Browsey". The bar
here is:

- bugs fixed as part of the Linux stabilization track are explicitly inventoried
- each listed fix points to at least one targeted automated regression test

## Inventory

| Stabilization bug/fix | Regression evidence |
|---|---|
| Cloud commands still worked when cloud was disabled in Settings | `list_cloud_remotes_impl_returns_cloud_disabled_when_toggle_is_off`, `validate_cloud_root_impl_returns_cloud_disabled_when_toggle_is_off` in `src/commands/cloud/mod.rs` |
| Local paste could leave completed work behind when a later source disappeared after clipboard capture | `paste_clipboard_copy_rolls_back_successful_items_when_later_source_fails`, `paste_clipboard_cut_rolls_back_successful_items_when_later_source_fails` in `src/clipboard/tests.rs` |
| Local directory paste could leave completed directory targets behind on cancel or later-source failure | `paste_clipboard_directory_copy_cancelled_after_first_item_rolls_back_created_targets`, `paste_clipboard_directory_cut_cancelled_after_first_item_restores_moved_source`, `paste_clipboard_directory_copy_rolls_back_successful_items_when_later_source_fails`, `paste_clipboard_directory_cut_rolls_back_successful_items_when_later_source_fails` in `src/clipboard/tests.rs` |
| Recursive overwrite-merge paths lacked rollback coverage on cancel and later failure | `paste_clipboard_overwrite_directory_copy_cancelled_after_first_merged_item_rolls_back`, `paste_clipboard_overwrite_directory_cut_cancelled_after_first_merged_item_rolls_back`, `paste_clipboard_overwrite_directory_copy_rolls_back_when_later_merged_source_fails`, `paste_clipboard_overwrite_directory_cut_rolls_back_when_later_merged_source_fails` in `src/clipboard/tests.rs` |
| Mixed local<->cloud transfers under-reported or mishandled active cancellation mid-batch | `mixed_execute_local_to_cloud_copy_cancels_during_second_active_transfer`, `mixed_execute_cloud_to_local_copy_cancels_during_second_active_transfer`, `mixed_execute_local_to_cloud_move_cancels_during_second_active_transfer`, `mixed_execute_cloud_to_local_move_cancels_during_second_active_transfer`, plus the progress-aware and directory variants in `src/commands/transfer/execute/tests.rs` |
| Extract could leave partial outputs behind or lacked direct real-archive conflict/cancel evidence | `extract_zip_rolls_back_partial_outputs_when_cancelled_mid_entry`, `tar_archive_is_readable_and_rolls_back_when_cancelled_mid_entry`, `do_extract_rolls_back_partial_outputs_when_cancelled_mid_entry`, `do_extract_uses_unique_destination_when_archive_root_conflicts`, `do_extract_extracts_real_archive_to_directory` in `src/commands/decompress/{zip_format.rs,tar_format.rs,mod.rs}` |
| Search/trash restore/purge failures could leave the UI in an ambiguous recovery state | `createSearchSession.test.ts` recovery cases and trash restore/purge failure coverage in `frontend/src/features/explorer/context/createContextActions.test.ts` |
| Advanced rename failures were not explicitly defended as a regression surface | `frontend/src/features/explorer/modals/advancedRenameModal.test.ts` |
| Local conflict preview lacked a direct preview-to-execute alignment assertion | `paste_clipboard_preview_matches_rename_execution_for_file_and_directory_conflicts` in `src/clipboard/tests.rs` and local conflict-routing coverage in `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts` |

## Conclusion

Based on the Linux stabilization fixes inventoried above, the current branch is
meeting the intended Step 7 bar:

- the meaningful stabilization bugs fixed in this track have targeted automated
  regression coverage
- the repo is no longer relying on incidental smoke coverage alone for these
  fixes

This is sufficient to close the Step 7 checklist item for the current Linux
production-ready track.

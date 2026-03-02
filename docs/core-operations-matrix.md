# Core Operations Trust Matrix

Created: 2026-03-02
Track: `docs/todo-archive/TODO_CORE_OPERATIONS_HARDENING.md`
Status: Source-of-truth matrix for trust-critical operation behavior.

This matrix is the canonical behavior definition for hardening work in this
track. Tests, manual validation rows, and release policy notes should reference
scenario IDs from this file instead of rewriting semantics elsewhere.

## ID Scheme and Stability Rules

Scenario format: `CO-<FAMILY>-<NNN>`

Families:

- `LCM`: Local clipboard-backed copy/move
- `LRN`: Local rename
- `LTD`: Local trash/delete/restore/purge
- `MTC`: Mixed local<->cloud transfer and conflict preview
- `EXT`: Archive extraction

Stability rules:

- Existing IDs are stable once published.
- Add new rows by appending new IDs; do not renumber existing rows.
- Deprecate by marking a row `Deprecated` in notes, not by deleting ID history.

## Ownership Map

| Code | Ownership scope | Modules |
|---|---|---|
| `OWN-LCM-BE` | Local clipboard backend orchestration | `src/clipboard/mod.rs`, `src/clipboard/ops.rs`, `src/clipboard/error.rs` |
| `OWN-LCM-FE` | Explorer clipboard/transfer UX | `frontend/src/features/explorer/file-ops/useExplorerFileOps.ts`, `frontend/src/features/explorer/services/clipboard.service.ts`, `frontend/src/features/explorer/services/transfer.service.ts` |
| `OWN-LRN-BE` | Local rename backend | `src/commands/rename/mod.rs`, `src/commands/rename/error.rs` |
| `OWN-LRN-FE` | Rename modal and dispatch | `frontend/src/features/explorer/modals/renameModal.ts`, `frontend/src/features/explorer/services/files.service.ts` |
| `OWN-LTD-BE` | Delete/trash backend routes | `src/commands/fs/delete_ops.rs`, `src/commands/fs/mod.rs`, `src/commands/fs/windows.rs`, `src/commands/fs/trash/mod.rs`, `src/commands/fs/trash/move_ops.rs`, `src/commands/fs/trash/listing.rs`, `src/commands/fs/trash/backend.rs` |
| `OWN-LTD-FE` | Delete/trash user flows | `frontend/src/features/explorer/modals/deleteConfirmModal.ts`, `frontend/src/features/explorer/components/DeleteConfirmModal.svelte`, `frontend/src/features/explorer/services/trash.service.ts`, `frontend/src/features/explorer/services/files.service.ts` |
| `OWN-MTC-BE` | Mixed transfer and cloud write orchestration | `src/commands/transfer/route.rs`, `src/commands/transfer/preview.rs`, `src/commands/transfer/execute.rs`, `src/commands/transfer/error.rs`, `src/commands/cloud/write.rs`, `src/commands/cloud/conflicts.rs`, `src/commands/cloud/events.rs`, `src/commands/cloud/providers/rclone/write.rs` |
| `OWN-MTC-FE` | Mixed transfer UI/progress/refresh | `frontend/src/features/explorer/file-ops/useExplorerFileOps.ts`, `frontend/src/features/explorer/services/transfer.service.ts`, `frontend/src/features/explorer/hooks/useExplorerData.ts`, `frontend/src/features/explorer/modals/cloudBackgroundRefresh.test.ts` |
| `OWN-EXT-BE` | Extraction backend | `src/commands/decompress/mod.rs`, `src/commands/decompress/error.rs`, `src/commands/decompress/util.rs`, `src/commands/decompress/zip_format.rs`, `src/commands/decompress/tar_format.rs`, `src/commands/decompress/seven_z_format.rs`, `src/commands/decompress/rar_format.rs` |
| `OWN-EXT-FE` | Extraction UX/progress | `frontend/src/features/explorer/file-ops/useExplorerFileOps.ts`, `frontend/src/features/explorer/services/files.service.ts` |

## Matrix

### Local clipboard-backed copy/move (`LCM`)

| Scenario ID | Flow | Happy path expectation | Conflict expectation | Cancellation expectation | Partial-failure expectation | Visible UI expectation | Ownership |
|---|---|---|---|---|---|---|---|
| `CO-LCM-001` | Local copy file | Destination file is created with source content preserved; source remains unchanged. | User sees conflict policy surface; selected policy is applied exactly once. | Cancel before commit keeps source/destination unchanged. | For multi-item copy, completed items stay copied and failures are reported per item. | Progress/toast reflects completion summary; listing refresh makes new file visible. | `OWN-LCM-BE`, `OWN-LCM-FE` |
| `CO-LCM-002` | Local copy directory | Recursive copy succeeds for all children within destination. | Existing destination children follow selected conflict policy (overwrite/rename/skip behavior as exposed by UI). | Cancel stops remaining work without deleting already copied data. | Partial completion is surfaced clearly; no silent drop of failed paths. | Explorer remains usable; summary distinguishes success/failure counts. | `OWN-LCM-BE`, `OWN-LCM-FE` |
| `CO-LCM-003` | Local move file | File appears at destination and is removed from source after success. | Conflict policy is honored deterministically and does not duplicate source unexpectedly. | Cancel before finalization keeps source intact. | If move degrades to partial state, UI reports unresolved/failed items clearly. | Source and destination views refresh to consistent state after operation ends. | `OWN-LCM-BE`, `OWN-LCM-FE` |
| `CO-LCM-004` | Local move directory | Directory tree ends in destination with source removed after success. | Conflict policy for existing destination tree is enforced predictably. | Cancel leaves clear state (remaining source entries retained, already moved entries visible). | Partial move is explicitly reported; no false "all done" signal. | Activity/progress closure and follow-up refresh reflect the actual result. | `OWN-LCM-BE`, `OWN-LCM-FE` |

### Local rename (`LRN`)

| Scenario ID | Flow | Happy path expectation | Conflict expectation | Cancellation expectation | Partial-failure expectation | Visible UI expectation | Ownership |
|---|---|---|---|---|---|---|---|
| `CO-LRN-001` | Rename file | File name updates atomically in current directory listing. | Existing target name is rejected or routed through explicit conflict behavior; no silent overwrite. | Cancel/escape closes rename intent without mutation. | Permission/read-only errors leave original name intact and return actionable message. | Selection/focus tracks renamed item when successful; error toast/modal when failed. | `OWN-LRN-BE`, `OWN-LRN-FE` |
| `CO-LRN-002` | Rename folder | Folder name updates and children remain accessible under new path. | Same-name target handling is explicit and deterministic. | Cancel keeps original folder name. | On failure, folder remains unchanged and navigation state stays valid. | Explorer refresh shows exactly one resulting folder name. | `OWN-LRN-BE`, `OWN-LRN-FE` |
| `CO-LRN-003` | Rename with conflict/no-overwrite semantics | Unique auto-rename (if chosen) generates non-colliding name. | No-overwrite path never destroys existing target content. | Cancel from conflict/rename modal keeps both original entries untouched. | Any failed fallback path is surfaced as failed, not silently coerced. | Modal closes with correct final status and refreshed listing. | `OWN-LRN-BE`, `OWN-LRN-FE` |

### Local trash/delete/restore/purge (`LTD`)

| Scenario ID | Flow | Happy path expectation | Conflict expectation | Cancellation expectation | Partial-failure expectation | Visible UI expectation | Ownership |
|---|---|---|---|---|---|---|---|
| `CO-LTD-001` | Trash file | File moves to trash backend and disappears from original folder. | Duplicate name in trash is handled by backend-safe naming/staging semantics. | Cancel before confirmation keeps file in place. | Failures (permission/path) keep source intact and are reported. | Listing refresh removes item; trash view can reveal moved item. | `OWN-LTD-BE`, `OWN-LTD-FE` |
| `CO-LTD-002` | Trash directory | Non-empty directory is moved to trash with contents preserved. | Name collision in trash does not cause silent data loss. | Cancel before destructive confirmation aborts operation. | Partial trash failures report failing path(s) and keep unresolved source entries. | UI summary communicates count and outcome; no stale selection artifacts. | `OWN-LTD-BE`, `OWN-LTD-FE` |
| `CO-LTD-003` | Restore from trash | Item is restored to original parent path or explicit fallback if original parent missing. | Name conflict at restore destination is resolved via explicit policy. | Cancel restore intent leaves trash state unchanged. | Failed restore leaves item in trash and reports why. | Source and trash views refresh consistently after result. | `OWN-LTD-BE`, `OWN-LTD-FE` |
| `CO-LTD-004` | Purge from trash | Item is permanently removed from trash inventory. | N/A (purge is destructive finalization). | Cancel before purge confirmation keeps item in trash. | Failed purge keeps item listed and reports error. | Trash listing reflects removal only on success. | `OWN-LTD-BE`, `OWN-LTD-FE` |
| `CO-LTD-005` | Permanent delete (bypass trash) | Target is removed permanently when user chooses delete semantics. | N/A (overwrite semantics not applicable). | Cancel before confirmation prevents delete. | Failure keeps target present and reports reason (permissions/in-use/etc). | Clear destructive confirmation and post-action refresh in current view. | `OWN-LTD-BE`, `OWN-LTD-FE` |

### Mixed local<->cloud copy/move + conflict preview (`MTC`)

| Scenario ID | Flow | Happy path expectation | Conflict expectation | Cancellation expectation | Partial-failure expectation | Visible UI expectation | Ownership |
|---|---|---|---|---|---|---|---|
| `CO-MTC-001` | Local -> cloud copy file | File is uploaded to cloud target; local source remains unchanged. | Conflict preview reflects target collision and applies selected strategy. | Cancel stops pending transfer and keeps source intact. | Partial outcomes are itemized; successful uploads remain, failed uploads are visible as failures. | Progress and completion summary remain consistent with backend result. | `OWN-MTC-BE`, `OWN-MTC-FE` |
| `CO-MTC-002` | Cloud -> local copy file | File is downloaded locally without mutating cloud source. | Local destination conflict behavior follows selected preview policy. | Cancel stops remaining transfer work cleanly. | Failed subset does not hide completed copies; errors are actionable. | Destination refresh shows only completed results; no phantom files. | `OWN-MTC-BE`, `OWN-MTC-FE` |
| `CO-MTC-003` | Local -> cloud move file | Upload succeeds then local source cleanup is applied according to move semantics. | Conflict strategy does not duplicate or lose source unexpectedly. | Cancel before source cleanup leaves local source intact. | If provider fails mid-operation, state is reported as partial and source cleanup is conservative. | UI makes "moved vs partially moved" state unambiguous. | `OWN-MTC-BE`, `OWN-MTC-FE` |
| `CO-MTC-004` | Cloud -> local move file | Download succeeds and cloud source deletion follows move semantics. | Conflict policy remains explicit and deterministic. | Cancel avoids deleting source entries that were not safely materialized locally. | Partial completion preserves auditability of what moved and what failed. | Source/destination refresh behavior does not imply false completion. | `OWN-MTC-BE`, `OWN-MTC-FE` |
| `CO-MTC-005` | Mixed directory copy/move | Recursive mixed transfer handles directory trees with deterministic routing. | Per-item conflicts across nested paths respect chosen policy. | Cancel stops remaining directory jobs and preserves already completed subpaths. | Summary clearly reports succeeded/failed path counts for large trees. | Activity panel and toast summary remain coherent for batch directory work. | `OWN-MTC-BE`, `OWN-MTC-FE` |
| `CO-MTC-006` | Mixed transfer conflict preview correctness | Preview enumerates collisions before execute and the same policy is carried into execute step. | Rename-on-conflict / overwrite / skip behavior matches previewed choice. | Cancel from preview executes nothing. | Any execute-time divergence from preview is surfaced as explicit error/warning. | Conflict modal closes into deterministic post-action state with refresh/update hooks applied. | `OWN-MTC-BE`, `OWN-MTC-FE` |

### Archive extraction (`EXT`)

| Scenario ID | Flow | Happy path expectation | Conflict expectation | Cancellation expectation | Partial-failure expectation | Visible UI expectation | Ownership |
|---|---|---|---|---|---|---|---|
| `CO-EXT-001` | Extract archive to new destination | All supported entries are materialized in expected destination layout. | N/A for new/empty destination. | Cancel before extraction starts produces no writes. | N/A for all-success path. | Progress closes with success summary and optional open-destination behavior. | `OWN-EXT-BE`, `OWN-EXT-FE` |
| `CO-EXT-002` | Extract archive into conflicting destination | Existing destination handling follows explicit overwrite/conflict semantics supported by extractor. | Conflicting paths do not silently clobber data outside chosen policy. | Cancel from conflict prompt aborts extraction before writes. | If some entries fail due to conflicts, successful entries remain and failures are listed. | Toast/modal summary distinguishes success/failure counts. | `OWN-EXT-BE`, `OWN-EXT-FE` |
| `CO-EXT-003` | Extract cancellation mid-run | Extract can be interrupted through supported cancel path without hanging UI. | N/A | Cancel stops remaining extraction work and closes progress channel cleanly. | Already-written files may remain; behavior is reported as partial/interrupted. | Explorer state remains interactive and error/progress state is cleared correctly. | `OWN-EXT-BE`, `OWN-EXT-FE` |
| `CO-EXT-004` | Extract permission-denied during write | Writable entries are extracted; denied paths fail with clear errors. | N/A | Cancel behavior same as `CO-EXT-003`. | Partial output is reported; operation does not claim full success. | User receives actionable failure message and can re-run safely. | `OWN-EXT-BE`, `OWN-EXT-FE` |
| `CO-EXT-005` | Multi-archive batch extraction | Batch extraction processes each archive independently and aggregates result summary. | Per-archive conflicts follow `CO-EXT-002` semantics. | Cancel stops remaining queue items and reports completed subset. | Mixed success/failure is surfaced per archive, not flattened. | Batch toast/progress summary reports exact success/failure totals. | `OWN-EXT-BE`, `OWN-EXT-FE` |

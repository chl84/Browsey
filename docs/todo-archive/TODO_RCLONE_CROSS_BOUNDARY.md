# TODO: rclone Cross-Boundary (Disk <-> Cloud)

Goal: support `local -> cloud` and `cloud -> local` for files/folders in Browsey without breaking existing `local <-> local` and `cloud <-> cloud` flows.

## V1 scope (locked)

- [x] Support `copy` and `move` between local disk and `rclone://...`
- [x] Support both files and folders (rename-on-conflict for mixed execute was pending when this list was active)
- [x] `stop-on-first-error` for batch (no rollback in v1)
- [x] No undo for cross-boundary in v1
- [x] No mixed selection (local + cloud in the same paste/drag) in v1

## Backend foundation

- [x] Create a dedicated cross-boundary transfer module (e.g. `src/commands/transfer/`)
- [x] Define clear request/response types for mixed preview + copy/move
- [x] Add Tauri commands for mixed conflict preview
- [x] Add Tauri commands for mixed `copy`
- [x] Add Tauri commands for mixed `move`
- [x] Reuse existing cloud path/parser/provider (`rclone`) where possible

## Conflict preview (robust + efficient)

- [x] Implement `local -> cloud` preview with a single cloud destination listing (not a `stat` loop)
- [x] Implement `cloud -> local` preview with a single local destination listing (not a `stat` loop)
- [ ] Keep rename-on-conflict (`-1`, `-2`, ...) with reservation in an in-memory set
- [x] Use provider-aware name matching for cloud targets (OneDrive case-insensitive)
- [x] Return a payload compatible with the existing conflict modal

## Execute (copy/move)

- [x] Implement `local -> cloud` file copy/move via `rclone` (files + folders)
- [x] Implement `cloud -> local` file copy/move via `rclone` (files + folders)
- [x] Verify and implement folder copy/move semantics explicitly (fake-rclone tests done; manual cloud verification remained)
- [x] Connect `overwrite` / `rename` policy to preview results (`prechecked=true` where possible)
- [x] Keep provider-aware error mapping for mixed operations

## Frontend routing + UX

- [x] Expand paste-route classification (`local`, `cloud`, `local_to_cloud`, `cloud_to_local`, `unsupported`)
- [x] Connect mixed conflict preview to the existing conflict modal
- [x] Connect mixed execute to the existing paste flow (`handlePasteOrMove`)
- [x] Keep correct activity labels (`Copying…` / `Moving…`) for mixed operations
- [x] Keep cut-clipboard clear after successful move (no half-tone regression)
- [x] Use background refresh/soft-fail where refresh is slow (especially cloud destination)

## Drag-and-drop and shortcuts

- [x] Enable `local -> cloud` drag-and-drop (in-app drag)
- [x] Enable `cloud -> local` drag-and-drop (in-app drag)
- [x] Keep blocking mixed selection in the same drag
- [ ] Verify `Ctrl+C` / `Ctrl+X` / `Ctrl+V` in both directions

## Robustness and performance

- [ ] Avoid unnecessary extra metadata calls in mixed preview/execute
- [ ] Reuse existing cloud cache/retry where it makes sense
- [x] Do not fail the whole operation if the write succeeds but refresh fails
- [x] Log enough timing/error context to debug quota/timeouts

## Tests

- [x] Backend tests for `local -> cloud` copy/move (file + folder)
- [x] Backend tests for `cloud -> local` copy/move (file + folder)
- [x] Backend tests for mixed conflict preview (rename/overwrite)
- [x] Frontend tests for route classification and conflict modal (mixed)
- [x] Frontend tests for `Moving…`/`Copying…` labels in mixed operations
- [x] Frontend tests for cut-state reset after mixed move

## Manual validation (OneDrive + Google Drive)

- [ ] `local -> cloud` copy file
- [ ] `local -> cloud` move file
- [ ] `local -> cloud` copy/move folder
- [ ] `cloud -> local` copy file
- [ ] `cloud -> local` move file
- [ ] `cloud -> local` copy/move folder
- [ ] Conflict flow (`rename` / `overwrite`) in both directions
- [ ] Drag-and-drop in both directions
- [ ] Shortcuts (`Ctrl+C`/`Ctrl+X`/`Ctrl+V`) in both directions
- [ ] Refresh/F5 fallback and user-facing errors on timeout/rate-limit

## Recommended PR order

- [ ] PR1: Mixed route + conflict preview (no execute)
- [ ] PR2: `local -> cloud` copy/move execute
- [ ] PR3: `cloud -> local` copy/move execute
- [ ] PR4: Drag-and-drop + UX wiring + refresh
- [ ] PR5: Tests + manual validation + docs

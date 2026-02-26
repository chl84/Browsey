# TODO: rclone Cloud Performance (OneDrive first)

## Phase 0: Baseline and measurement

- [x] Add simple timing/logging for cloud operations (`conflict preview`, `write`, `refresh`)
- [ ] Define and run a baseline test in the OneDrive test folder (navigation, copy, rename, delete)
- [ ] Record before/after timings for each optimization

## Phase 1: Refresh performance (high UX impact)

- [x] Cloud paste: background refresh instead of blocking the entire operation
- [x] Introduce a refresh coordinator (single-flight) per cloud folder
- [x] Debounce/coalesce multiple refresh requests close together
- [x] Use the same background refresh pattern for cloud `rename`
- [x] Use the same background refresh pattern for cloud `mkdir`
- [x] Use the same background refresh pattern for cloud `delete`
- [x] Use the same background refresh pattern for cloud `move`

## Phase 2: Fewer rclone calls in paste/conflict flows

- [x] Change `preview_cloud_conflicts` to a single destination-folder listing (not `stat` per source)
- [x] Build the conflict set in memory from the destination listing
- [x] Remove the `statCloudEntry` loop in frontend rename-on-conflict
- [x] Avoid duplicate overwrite precheck when conflict preview has already been performed
- [ ] Verify that cloud paste makes fewer `lsjson/--stat` calls in logs

## Phase 3: Caching for navigation and Network view

- [x] Cache cloud remote discovery (`listremotes + config dump`) with TTL
- [x] Invalidate remote cache on explicit Network refresh
- [x] Cache cloud directory listings (`rclone://...`) with short TTL
- [x] Invalidate listing cache on cloud write operations (copy/move/rename/mkdir/delete)
- [x] Reduce duplicate cloud listings in `list_facets` (use cache or existing entries)

## Phase 4: Robustness under slowness / throttling

- [x] Bounded concurrency per remote (avoid too many concurrent rclone calls)
- [x] Short retry/backoff for metadata/listing (`lsjson`) on transient failures
- [x] Improve cloud UX messages during slow refresh (do not report write as failed)
- [ ] Verify behavior during several rapid operations in a row

## Phase 5: Tests and validation

- [x] Backend tests for batch conflict preview
- [x] Backend tests for cache TTL + invalidation
- [x] Frontend tests for refresh coordinator / coalescing
- [x] Frontend tests for background refresh on rename/mkdir/delete
- [ ] Manual OneDrive performance test (same checklist as baseline)
- [x] Update `TODO-rclone.md` with performance status when the first package is complete

## Recommended PR order

- [x] PR1: Refresh coordinator + background refresh for cloud write operations
- [x] PR2: Batch conflict preview + remove `stat` loop in rename-on-conflict
- [x] PR3: Remote discovery cache (Network view)
- [x] PR4: Cloud listing cache + invalidation
- [x] PR5: Cloud facets without duplicate listing
- [x] PR6: Bounded concurrency + metadata retry/backoff

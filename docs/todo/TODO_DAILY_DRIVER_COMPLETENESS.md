# Browsey Daily-Driver Improvement TODO

Created: 2026-10-03
Reorganized: 2026-10-03
Evidence updated: 2026-10-08, after Google Drive object-identity implementation and candidate acceptance.
Status: Active engineering follow-up; acceptance is tracked separately.
Baseline: Browsey 1.0.3, verified implementation through `0bf3ff6`.

Goal: Improve dependable daily use within an explicit Linux support scope.
This list contains engineering follow-ups and reproduced validation findings,
not a count of missing file-manager features. Test execution, release procedures
and unapproved product proposals
are not implementation TODOs.

## Where Progress Is Tracked

- [Verified work archive](../todo-archive/TODO_DAILY_DRIVER_SAFETY_COMPLETED.md):
  33 already-checked coverage/safety increments and their audit evidence.
- [Daily-driver validation checklist](../operations/linux-release/daily-driver-validation-checklist.md):
  outstanding installed-build, platform, device, UI, accessibility and remote checks.
  An unchecked test means unverified, not unimplemented or a confirmed bug.
- [Completed native suite](../todo-archive/TODO_NATIVE_TEST_SUITE.md) and
  [case-to-acceptance index](../operations/linux-release/native-case-acceptance.md):
  bounded native candidate evidence; completion does not close broader installed,
  device or provider acceptance requirements.
- [README](../../README.md) and [1.0.3 release notes](../releases/1.0.3.md):
  existing capabilities, published scope and release-specific limitations.

Browsey already implements copy/move, undo, archive passwords, shared controls,
themes, thumbnails, Open With, USB formatting, MTP discovery and rclone support.
Inspect current code and evidence before adding or replacing behavior.

## Priority 0 Safety and Recovery

- [x] Fix Google Drive identical-name object identity or refuse ambiguous
  operations safely. The installed
  [provider-difference run](../operations/linux-release/runs/2026-10-07-google-drive-provider-differences.md)
  reproduced two object IDs collapsing into one 47-byte row, whose native
  download returned the other object's 24 bytes twice. Preserve object identity
  through listing, selection and operations; a row-key-only fix is insufficient.
  The [2026-10-08 candidate run](../operations/linux-release/runs/2026-10-08-google-drive-object-identity.md)
  verifies separate rows, exact selected bytes, independent folder navigation,
  ID-preserving rename, selected-object deletion and unique-target overwrite.
  Focused regressions cover ambiguous bulk/overwrite refusal and shortcut/native
  document behavior. General file handling retains its existing hot paths;
  Google directory transfers alone add a safety preflight. This closes the
  engineering defect, not the release or broader C6 provider acceptance.

- [ ] Diagnose the reproduced MTP hidden-folder deletion failure and establish
  a safe provider/device workaround or supported boundary. The
  [Hidden/delete follow-up](../operations/linux-release/runs/2026-10-07-hidden-delete-cleanup.md)
  separates Browsey's corrected stale Properties paths from direct-GIO and
  Nautilus failures, including a hidden folder never renamed. Do not replay
  deletion or accept a provider success receipt without checking the result.

- [x] Review the remaining concurrent-writer/final-check windows and document
  the smallest justified mitigation or explicit supported boundary. Use the
  [content-verification audit](../audits/daily-driver/copy-content-verification.md)
  and existing deterministic tests; readback and metadata receipts do not make
  trees transactional. Do not assume staging, persistent undo or automatic
  resume is an approved solution. Acceptance of observed outcomes stays in
  checklist A0. The [boundary decision](../audits/daily-driver/concurrent-writer-boundary.md)
  records the existing checks, remaining windows and requirement to stop other
  writers. Two installed real-writer scenarios and deterministic removal tests
  support the limited claim; A0-7 remains partial. No transaction, staging or
  automatic recovery engine was added.
- [x] Measure representative backup sizes and define a safe recovery-storage
  budget/retention policy before implementing any missing controls. The
  [storage audit](../audits/daily-driver/undo-storage-diagnostics.md) and
  [undo scope](../operations/linux-release/undo-scope.md) document the concrete
  gap: read-only diagnostics and a 50-action history cap are not a byte quota.
  Review existing cache/log limits and settings feedback rather than adding
  duplicate mechanisms. Never automatically purge protected recovery sessions,
  live operations or files still needed by open programs. Keep maintenance-agent
  caches separate; installed recovery UX is checklist A0.
  The [measurement and policy](../audits/daily-driver/recovery-storage-policy.md)
  records four production-engine workloads on tmpfs/Btrfs, backup reuse,
  history eviction without disk reclamation and a conservative provisioning
  rule rather than an unsafe fixed quota. Existing Settings diagnostics now
  separately report filesystem allocation on Unix, retaining partial-scan and
  CoW caveats. Strict maintenance passed: 645 backend tests, 8 opt-in tests
  ignored, 318 frontend tests, 62 browser tests, Clippy/Semgrep and 20 docs checks.
  The disposable recovery measurement passed separately on both filesystems;
  no automatic purge, installation or new native acceptance is claimed.

## Priority 1 Reproducible Performance Work

- [x] Create reproducible disposable workloads for 10,000 and 100,000 entries,
  mixed thumbnail formats, recursive search, and controlled slow storage.
  Reuse existing thumbnail and cloud performance tests before adding harnesses.
  The [workload guide](../audits/daily-driver/performance-workloads.md) records
  five opt-in production-engine workloads, mock-UI tooling and an isolated
  native candidate observer. Runner safety and virtualization regressions pass.
- [x] Measure generated OS-warm listing/search, mixed decoder/cache paths,
  mock thumbnail display/scroll/zoom, controlled cancellation, native
  fresh/repeated-profile startup, memory and owned disk-cache growth. The guide
  records hardware/data/OS/source/binary and five-sample median/max, not p95.
  Removing unused local caching reduced the 100k listing median 840 → 537 ms;
  the network cache is bounded to 10k entries. Native WebKit file accessibility
  was corrected and verified. These measurements do not prove physical cold I/O.
  The [2026-10-04 Ctrl-wheel follow-up](../audits/daily-driver/ctrl-wheel-zoom.md)
  adds rapid-input/layout/thumbnail fixes and same-browser before/after zoom
  measurements at 10k/100k entries. Its automated interaction/density subtests
  are recorded under A2-1/A2-3 in the
  [validation checklist](../operations/linux-release/daily-driver-validation-checklist.md#a2-interaction-and-feedback);
  installed WebKit, real thumbnail I/O and the open performance rows below
  are not signed off by those mock measurements.
- [x] Measure bounded local native listing, first displayed thumbnails, zoom,
  drag feedback and cancellation. The
  [NT7-3 record](../operations/linux-release/runs/2026-10-07-native-repeatability.md#nt7-3-measured-bounded-local-baseline)
  records a 50-entry workload, 128 ms to the first decoded thumbnail and actual
  input/UI observations. The private app/thumbnail cache was fresh, while the
  generated files and OS page cache were warm. These are descriptive observations,
  not cold-storage measurements or agreed performance budgets.
- [x] Measure bounded native opening and first decoded thumbnails on local,
  USB, MTP and OneDrive with independent source-byte preservation and explicit
  cache-state evidence. The
  [daily-driver follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#storage-opening-and-thumbnails)
  records five samples per provider, local/USB source-data eviction, and
  uncontrolled device/provider caches. It does not close the broader cold/slow
  storage rows below; the maintainer-approved budget scope is defined separately.
- [ ] Measure real cold/slow-storage opening and first displayed native
  thumbnails on representative local storage.
  Bounded five-sample native opening/decoded-thumbnail measurements and verified
  source-data page eviction now pass in the
  [daily-driver follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#storage-opening-and-thumbnails).
  Directory metadata and physical firmware cold state remain uncontrolled.
- [ ] Measure opening and first displayed native thumbnails on representative
  cold/slow USB storage within an approved device scope.
  The same record adds five completed USB samples, verified source-data page
  eviction and independent preservation; physical cold device state is unverified.
- [ ] Measure cold/slow-storage opening and first displayed native thumbnails
  on representative MTP inputs within an approved device scope.
  Five native samples and independent image bytes pass on the approved phone.
  This small generated workload was fast; device/GVFS caches and a controlled
  slow-device condition remain unverified.
- [ ] Measure cold/slow-storage opening and first displayed native thumbnails
  on representative cloud inputs within an approved provider scope. Functional
  thumbnail tests are not representative storage-performance measurements.
  Five real OneDrive samples and independent readbacks pass, including a 9.2 s
  first-thumbnail outlier. Provider caches are uncontrolled; the linked record
  distinguishes slow real network I/O from fully cold provider state.
  Mock display, fresh profiles and fresh thumbnail caches do not substitute for
  real cold/slow-storage evidence in these four remaining rows.
- [x] Record an initial host-specific native timing baseline and its measurement
  boundaries. NT7-3 separates delivered-pointer/layout observations and actual
  cancel acknowledgement/completion from driver overhead and toast expiry.
  No hard CI timing thresholds were introduced.
- [x] Agree measured performance budgets for the bounded storage workload;
  investigate the slowest path first.
  Add regression thresholds only where the environment is stable enough to
  avoid flaky CI. Do not assume that more workers improve MTP/cloud performance.
  The maintainer approved five-sample median review targets on this host:
  local/USB/MTP opening and first decoded thumbnail ≤200 ms each; OneDrive
  opening ≤3 s and first thumbnail ≤5 s. These apply to 12 generated PNGs and
  thumbnails timed from the grid toggle. All measured medians meet the targets;
  the 9.2 s OneDrive maximum remains visible. The
  [follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#slowest-path-and-budgets)
  investigates the slowest cloud path and its cache/network boundaries.
  Broader physical-cold and mixed-large-tree budgets remain separate; earlier
  warm engine targets stay provisional. CI uses semantic/structural checks,
  not workstation latency gates.

These measurements also cover the extra output read pass introduced by local
copy content verification. The existing warm-tmpfs result is evidence for that
workload only, not a cold-storage or USB/MTP performance budget.

## Maintainability Follow-up

Added: 2026-10-04. Work through these four review findings in order; extend
existing modules rather than reopening completed refactoring tracks.

- [x] **M1** Share copy, cut and deletion action flows between keyboard
  shortcuts and context menus, preserving confirmation, progress and cancellation.
  Resolved 2026-10-04 with `createSelectionActions`, reused by both entry points.
  Regression coverage includes selection resolution, clipboard sync failures,
  network confirmation, cancellation, duplicate requests and partial-result
  refresh without mutation retry. Refresh warnings cannot be overwritten by
  success feedback; direct Wastebasket purge offers no unsupported cancellation.
  Verification: 29 focused regressions; all 510 frontend tests and 99 mock-browser
  tests, lint/typecheck/build pass. Strict maintenance/docs checks pass, including
  728 backend tests (18 opt-in tests ignored) and 20 documentation checks.
  The final purge-cancellation adjustment was rechecked with the focused tests,
  lint/typecheck/build. Native WebKit acceptance remains separate.
  Final review also reproduced and fixed confirmation opening before previous
  progress cleanup finished. The sequencing regression brings focused coverage
  to 30 tests; lint/typecheck/build and all 103 mock-browser tests pass.
- [x] **M2** Replace `any` in `createExplorerShellProps.ts` with concrete
  types for values, modal state and callbacks.
  Resolved 2026-10-04. Assembly uses the consuming component's prop contract and
  existing modal/controller types; the page and ExplorerShell no longer bypass
  those contracts with `any` prop bags. Explicit value types avoid narrowing
  initial state to `false`/`null`. Three contract tests include five invalid
  assignments that must remain compile errors. All 513 frontend tests and
  99 mock-browser tests, lint/typecheck/build pass. No runtime UI changes;
  native acceptance is not inferred from the mock-browser checks.
- [x] **M3** Extract USB formatting, shortcut registration and file-operation
  orchestration from `ExplorerPage.svelte` into existing or focused modules.
  Keep the page as the composition root and preserve user-facing behavior.
  Resolved 2026-10-04 with `createUsbFormatModal`, `createExplorerShortcuts` and
  `createExplorerFileActions`. The page remains the composition root; the
  existing keyboard router, shared selection mutations and service boundaries
  remain authoritative. USB inspection/progress cannot overwrite newer requests,
  refresh failures do not reclassify erase outcomes, and completed dialogs can
  close while watcher refresh finishes. Cloud-open listeners are released.
  Verification: 52 focused controller regressions, all 565 frontend tests and
  103 mock-browser tests, lint/typecheck/build pass. Four new browser regressions
  cover filter/backspace, rename/Escape, fresh Properties selection, console and
  select-all after navigation. No physical drive was formatted; native/device
  acceptance remains separate.
- [x] **M4** Reuse shared error normalization in the Properties modal,
  preserving supported error shapes and permission-specific messages.
  Resolved 2026-10-04. Properties uses shared `getErrorMessage`/`getErrorCode`
  rather than parallel parsers. Normalization retains nested/JSON-encoded typed
  IPC codes, diagnostic metadata and immutable Error identity/stack; codes are
  never inferred from prose. Permission/ownership-specific messages and previous
  UI state remain intact after failed requests, without mutation retries.
  Verification: 84 focused error/IPC/Properties tests, including four Properties
  cases reproduced before the fix. Final combined strict maintenance run passes:
  728 backend tests (18 unchanged opt-in tests ignored), 621 frontend tests,
  103 mock-browser tests, Rustfmt/Clippy/Semgrep, frontend lint/typecheck/build
  and 20 documentation checks. Native WebKit and physical USB formatting were
  not exercised by these checks.

After each item, run relevant regression tests and frontend lint, typecheck
and build. Check the item only when verified and record the results; native
acceptance remains in the separate validation checklist.

## Confirmed Validation Findings

- [x] **VD-1** Correct the stale Settings claim that compressed RAR entries are
  unsupported, including its matching filter text. The
  [Omarchy validation run](../operations/linux-release/runs/2026-10-03-a0-omarchy.md)
  reproduced the message in both the installed app and mock browser while ten
  RAR tests passed. Update existing wording and add a focused regression; do not
  replace the working archive adapter. Resolved with shared Settings/filter copy
  describing compressed/password extraction and unsupported RAR creation. Four
  focused regressions, all 300 frontend tests, lint/typecheck/build, ten RAR tests
  and the updated mock-browser Settings view pass. The later
  [native candidate follow-up](../operations/linux-release/runs/2026-10-03-a0-errors-and-writers.md)
  also verified the wording and password filter in the optimized app. The
  installed binary has not been replaced. This was a non-blocking
  capability-message defect, not a missing extraction feature.
- [x] **VD-2** Report failed archive names/reasons in partial batch extraction;
  preserve the real operation outcome when the subsequent listing refresh fails.
  The [native error/writer run](../operations/linux-release/runs/2026-10-03-a0-errors-and-writers.md)
  reproduced count-only feedback for one valid and one broken ZIP. Four new
  regressions failed before the fix, including incorrect extraction-failure
  feedback after successful work and missing refresh after extraction failure.
  Reuse the existing toast/activity/listing infrastructure: show bounded failure
  details, distinguish refresh failure with F5 guidance, and refresh after error
  or cancellation without retrying extraction. Six new regressions pass, as do
  all 306 frontend tests; the optimized candidate's native partial-result message,
  contents and refreshed listing also pass. This fixes reporting, not archive
  atomicity, automatic retry or a new persistent operation history.
- [x] **VD-3** Resolve the four typed-error seams found after installing Semgrep
  and make the nominally blocking gate fail on findings. Preserve existing IPC
  codes with explicit I/O/runtime/task conversions; do not reclassify typed task
  errors from diagnostic text. Local and CI now reuse the
  [scan runner](../../scripts/maintenance/check-semgrep.mjs) and four fixture
  regressions for forbidden patterns, exit codes, exclusions and invalid config.
  Both real scans are clean (213/147 files); all 620 backend tests and
  warnings-denied Clippy pass, with five opt-in tests ignored. Rules and
  allowlists are unchanged. See the
  [guard procedure](../ERROR_HARDENING_EXCEPTION_POLICY.md#running-the-semgrep-guards).
  This is local verification, not a new GitHub CI or installed-app signoff.

- [x] **VD-4** Fix OneDrive directory copy through native clipboard paste.
  The [scoped native foundation run](../operations/linux-release/runs/2026-10-05-native-foundation.md)
  copied an individual generated file correctly, then reported a cloud-operation
  failure for a directory containing one generated child. Independent read-only
  inspection found the destination directory absent and the source child intact.
  Reuse the existing cloud transfer adapter and verify file/tree behavior;
  do not automatically retry an uncertain write.
  Resolved in the [native foundation follow-up](../operations/linux-release/runs/2026-10-05-native-foundation-fixes.md):
  source metadata routes directories to the existing directory-aware CLI before
  writing. File/tree copy and move within OneDrive passed through native controls,
  with independent byte and source-preservation/removal checks. This does not
  establish conflict, cancellation, disconnect or large-tree acceptance.
- [x] **VD-5** Handle unsupported directory-permission preservation on MTP and
  keep operation completion visible until the final backend result.
  The same [native run record](../operations/linux-release/runs/2026-10-05-native-foundation.md)
  records a folder-copy failure setting permissions (`Operation not supported`,
  95), with a retained non-empty destination (39). The failure arrived after
  destination content was visible and the activity indicator had disappeared.
  Preserve source/recovery data and truthful failure feedback; verify generated
  folder copies and moves before accepting mobile or either local/mobile route.
  Resolved in the [native foundation follow-up](../operations/linux-release/runs/2026-10-05-native-foundation-fixes.md):
  explicit Unsupported permission preservation is non-fatal; other errors still
  fail. Paste stays busy until the command reply. The 17-case local/mobile run
  passed within-provider and both-direction file/tree transfers, plus teardown.
  Broader cancellation/disconnect and directory-size stress acceptance remains open.

- [x] **VD-6** Enable verified fallback moves between local storage and GVFS/MTP
  without weakening source-retention checks. The
  [native foundation follow-up](../operations/linux-release/runs/2026-10-05-native-foundation-fixes.md)
  reproduced a safe refusal of local-to-mobile source removal because the GIO
  writer provided no ownership receipt. Select an exclusive owned writer before
  copying, verify bytes/identity, then remove only unchanged source entries.
  Failed readback or missing receipts must still preserve the source; never
  retry an uncertain write just to obtain a receipt.
  Resolved with the owned stream writer selected before a fallback move copy.
  Writer identity and content verification provide the receipt; the missing-
  receipt/source-retention guard remains active. New failure regressions pass,
  and native local/mobile file/tree moves passed in both directions with verified
  source removal and unchanged destination bytes. No uncertain write was retried.

- [x] **VD-7** Remove empty source directories after mixed local/cloud moves
  while preserving empty destination directories. The five-provider rerun in the
  [native foundation follow-up](../operations/linux-release/runs/2026-10-05-native-foundation-fixes.md)
  copied the nested bytes correctly but retained the empty local source tree.
  Use rclone's directory `move` route with its empty-directory flags before any
  write, preserving existing file routes and cancellation/source-retention rules.
  Correct the fake provider's cross-backend directory semantics and verify native
  file/tree moves in both directions before checking this item.
  Resolved with directory-aware moves and a final empty-only source-root removal
  after destination confirmation. Local source identity is captured before I/O
  and rechecked; changed/nonempty/cancelled roots are retained. All 39 activated
  mixed-transfer tests pass, including these failure cases. Native file/tree moves
  local → OneDrive and OneDrive → local both passed with independently verified
  source removal and destination bytes in the final five-provider run.

## Approved Cloud Integration Expansion

Approved on 2026-10-03. Reuse the shared provider, transfer, archive, task and UI
layers. Engineering completion and real-provider acceptance are separate.

- [x] **C1** Durable, private working copies; protect open edits from preview
  eviction, cache clearing and subsequent opens; retain failed work across restart.
- [x] **C2** Provider-aware normal trash and explicit permanent deletion;
  explain recovery through the provider website and unsupported providers.
- [x] **C3** Cloud new file, Open With and explicit upload of edited working
  copies; detect source changes and use Save as new when atomic overwrite is unavailable.
- [x] **C4** Compress/extract using protected local staging and the existing
  archive engine, password prompts, cancellation and progress; retain originals
  and recoverable working data on failure (the shared archive engine may clean
  up its own unsuccessful partial output).
- [x] **C5** Advanced rename, prepared external copy-only drag and consistent
  refresh; expose only supported actions through the existing capability model.
- [ ] **C6** Disposable acceptance on OneDrive, Google Drive and Nextcloud:
  conflicts, network loss, quota, concurrent edits, large trees and cancellation.
  Automated fixtures are not real-account acceptance; personal data is out of scope.
  - [x] OneDrive backend acceptance: durable edits, unique upload, same-size
    source changes, existing-target refusal, pre-cancellation, advanced rename,
    encrypted ZIP round trip and owned-child trash cleanup (2026-10-03,
    rclone 1.75.1; opt-in workspace acceptance test).
  - [x] OneDrive extracted-tree round trip: encrypted ZIP, 32 files/eight groups,
    nested empty directories, exact bytes, original archive retained and an
    occupied directory refused. This is functional coverage, not a large-tree budget.
  - [x] OneDrive active cancellation and process-scoped network interruption:
    positive real rclone byte statistics observed before fault injection;
    cancellation/network error classified, source bytes preserved, owned-child
    normal trash cleanup verified. No global network or quota manipulation.
    Revalidated after the OData-URL privacy fix on `ed109fb`: cancellation
    returned in 25.1 ms and network failure in 50.2 ms after positive byte
    statistics; signed URLs were absent from feedback and the parent was empty.
  - [x] OneDrive native archive UI/IPC/staging acceptance: context-menu password
    ZIP creation and password-modal extraction, refreshed output, exact bytes
    and nested empty directories, unchanged source/archive and scoped cleanup.
    Revalidated with the production candidate built from `ed109fb` after the
    RC completion/no-retry fixes, not just the earlier native checkpoint.
  - [x] OneDrive bounded post-preflight copy conflict: a same-size competing
    test file appears before rclone starts; CLI and RC progress preserve its
    bytes and the source, and a skipped copy is not reported as successful.
    A legitimate zero-byte RC upload also succeeds and is verified remotely.
    This is not provider CAS or protection against a later in-flight race.
  - [x] OneDrive bounded larger/deeper archive tree: 512 files in eight groups,
    depth eight, nested empty directories, exact round-trip bytes, original
    archive retained and occupied target refused. Tree upload/download took
    207/77 seconds within existing transfer limits. All five real-provider
    tests passed serially on `ed109fb` in 810 seconds with owned-child cleanup.
    This is not an unlimited-scale or mixed-large-file performance budget.
  - [x] OneDrive native refresh, upload, cloud copy, download and file conflicts:
    Cancel, Skip, Auto-rename and Overwrite, with independent exact-byte checks.
    [NT5-4](../operations/linux-release/runs/2026-10-06-native-provider-behavior.md#nt5-4-onedrive-operations-working-copies-and-bounded-errors)
    also verifies working-copy preparation, Save as new and detection of an
    equal-size changed original. External-editor launch was outside this scope.
  - [x] OneDrive bounded native quota/rate/authentication error feedback and
    source retention: NT5-4 injects failures before writes and verifies unchanged
    sources/cloud trees, truthful counts and released tasks. This checks the
    application response, not real account quota, throttling or a live outage.
  - [x] OneDrive native archive staging round trip and archive-name conflict:
    [NT6-7](../operations/linux-release/runs/2026-10-07-native-desktop-interaction.md#nt6-7-generated-archives-and-actual-cloud-staging)
    verifies actual cloud compression/extraction, refreshed output, exact bytes
    and preservation of both originals when the archive destination is occupied.
  - [x] OneDrive prepared cloud-file export to a native external GTK receiver.
    The [daily-driver follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#prepared-cloud-copies-to-nautilus)
    records actual isolated Nautilus copy, cancelled drag, special filenames and
    independent cloud/staging/receiver bytes. Private X11 copy does not certify
    shared Wayland receivers or move semantics.
  - [x] OneDrive bounded native mixed-size tree upload/download: 1028 files,
    16 directories, eight empty leaves and 11.4 MB of generated bytes. The
    [daily-driver follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#mixed-size-tree-and-transfer-deadline-correction)
    records independent cloud SHA-256 and complete source/download preservation.
    The fixed five-minute transfer deadline was corrected to an inactivity
    limit; the fresh upload/download workflows completed in 408/428 seconds.
    Earlier partial data and the failed report remain retained.
  - [ ] OneDrive repeated broader-scale/mixed-large-file performance measurements
    and agreed budgets; single complete-workflow observations do not establish
    distribution estimates or these budgets.
  - [ ] OneDrive dedicated-account real quota/rate-limit acceptance. The
    pre-write injected errors above do not close this provider requirement.
  - [x] OneDrive server-side concurrent destination race: the
    [daily-driver follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#server-side-destination-writer)
    records an actual competing rclone write after upload and before client
    completion, with independent bytes. This bounded checkpoint supports the
    stable-destination boundary; it does not certify arbitrary during-upload
    writers, compare-and-swap or transactional tree operations.
  - [x] OneDrive web recycle-bin restore of generated test data. The
    [daily-driver follow-up](../operations/linux-release/runs/2026-10-07-daily-driver-followup.md#native-trash-and-manual-web-restore)
    records real native upload/trash, maintainer-confirmed web restoration and
    independent exact original-path/size/SHA-256 verification of one generated
    file. No personal data or bin-wide action was used.
  - [x] Google Drive bounded installed-build acceptance: the
    [production run](../operations/linux-release/runs/2026-10-07-google-drive-production.md)
    verifies native copy/move, an 8 MiB upload, folders, Open With, durable edits
    after cache clearing/restart, unique edited upload, encrypted ZIP creation/
    extraction with empty directories, normal trash and permanent delete. Only
    generated, ownership-marked data was used and cleaned up.
  - [x] Google Drive installed provider differences: the
    [difference run](../operations/linux-release/runs/2026-10-07-google-drive-provider-differences.md)
    verifies case-sensitive upload/rename, exact-name collision refusal,
    advanced rename, native-document unknown size/DOCX/Open With and shortcut
    copy/trash/download. Identical-name objects failed in that installed binary;
    the subsequent candidate fixes the Priority 0 defect above. Neither run
    closes C6.
  - [x] Google Drive object-identity production candidate: the
    [candidate run](../operations/linux-release/runs/2026-10-08-google-drive-object-identity.md)
    records selected-ID native operations, explicit ambiguous bulk/overwrite
    refusal, 856 backend and 723 frontend regressions and a local-listing
    measurement. The installation remains unchanged.
  - [ ] Google Drive remaining expanded-provider cases: changed sources/concurrent targets, active cancellation/network
    failures, quota/rate limits, external drag, web restore and scale budgets.
    Reuse shared OneDrive/rclone evidence for common workflows and prioritize
    Google-specific behavior rather than repeating the complete OneDrive suite.
  - [ ] Nextcloud: obtain an approved disposable remote/folder and run
    the expanded real-provider checklist.

C1–C5 mark implemented engineering scope, not a new production release or
complete provider acceptance. Regression coverage reuses the shared provider,
transfer and archive suites; new workspace tests cover persistence, permissions,
same-size edits, manifests and reserved names. Cloud upload fixtures cover
network/quota failure retention, retry, active cancellation and missing parents.
Frontend tests cover cloud Open With/local paths, typed archive errors, routing,
recursive capabilities and staged activity lifetime. UI automation separately
covers recovery while cloud is off and prepared copy-only export. No automatic
upload, cloud undo, provider CAS overwrite or cleanup is claimed.

Verification on 2026-10-03: strict maintenance suite passed (641 backend tests,
6 opt-in tests ignored in the ordinary run; 317 frontend tests; 62 browser
tests). Clippy deny-warnings, blocking Semgrep, both frontend/docs builds and
20 strict documentation checks passed. The real OneDrive opt-in test was run
separately and passed; native cloud-export external-receiver acceptance remains
pending.

Native follow-up on 2026-10-07: the archived NT0–NT7 suite and linked run records
add bounded real WebKitGTK/Rust evidence to the checked subtasks above. C6 remains
open for the listed OneDrive requirements, remaining Google Drive cases and
separately approved Nextcloud acceptance. Real provider lifecycle transitions and external CI execution
remain NOT_RUN; broader cold-device/mixed-tree budgets remain NOT_AGREED.
The bounded 12-image storage median targets above are maintainer-approved.
No broader parent acceptance requirement is closed by this evidence update.

## Optional Decisions, Not Required Work

These proposals remain unapproved and have no completion checkboxes. Confirm
current support and user value before selecting an implementation. Cloud parity,
localization and additional platforms are not prerequisites for local file safety.

- Decide localization scope. If approved, centralize user-facing strings
  and add English/Norwegian translations with plural, date, and size formatting
  tests. Localization is not assumed implemented or mandatory for Linux file safety.
- Evaluate tabs and session restoration; review state isolation, dirty
  operations, keyboard shortcuts, and memory before committing to implementation.
- Evaluate a split view for frequent copy/move workflows; define independent
  selection, focus, navigation, destination preview, and transfer ownership.
- Evaluate quick file preview and archive browsing without extraction.
  Bound memory/I/O and sandbox untrusted content; never execute previews.
- Evaluate a transfer queue or operation history. Distinguish visibility and
  scheduling from resumable transfers; not every operation can safely pause/resume.
- Decide whether to deepen Windows validation or keep maintenance-only scope.
  macOS support and new Linux package formats require separate plans.
- Add an opt-in diagnostic export only if needed: version/session, safe
  dependency status, and redacted errors. Preview exported data and exclude
  credentials, archive passwords, personal paths, and communication by default.

## Working and Completion Rules

- For every selected item, record evidence of the gap, the smallest fix,
  changed files, exact checks/results, remaining risks, and tested commit.
- Keep fixes small and independently verifiable. Reuse shared UI, error,
  task, and capability infrastructure rather than adding parallel mechanisms.
- Pass `bash scripts/maintenance/test-both.sh --strict-docs` and relevant
  security/native checks for implementation changes; use strict docs consistency
  and local-link/requirement checks for documentation-only reorganization.
  Report environmental skips without calling them passes.
- Complete fresh manual acceptance for affected device/platform workflows.
  Keep release-specific outstanding checks in the relevant release notes.
- Update user-facing support/limitation docs and make a release through the
  existing bump, package verification, publication, and install procedure.
  Do not overwrite historical tags, release files, or validation results.
- After the required phases pass, run a maintainer-approved stabilization
  period in ordinary use and record unresolved defects before final signoff.
- Archive the active plan when selected work is complete; move explicitly chosen
  optional expansions into separate active plans rather than leaving it open forever.

This documentation cleanup does not authorize installation, restart, release,
dependency upgrades, destructive tests or tests against personal cloud data.
Completeness signoff still needs fresh evidence for the agreed scope, no unresolved
release-blocking trust bugs, coherent accessible workflows and honest capability
boundaries. Closing engineering tasks does not automatically pass acceptance.

## Reclassification Record

The former list had 33 checked and 48 unchecked rows. The checked rows are
archived unchanged. Of the unchecked rows, 29 are acceptance checks, four are
engineering work (three performance rows and retention), eight are optional
decisions, and seven are ongoing/release rules. The mixed recovery row keeps UX
acceptance in the checklist and storage-policy work here. The concurrent-writer
validation row remains a test requirement; its documented design boundary is
also an explicit engineering follow-up. At reclassification there were five active
TODO boxes, not five newly discovered defects. Later reproduced findings are added
separately above. No unchecked requirement was silently dropped
or declared verified.

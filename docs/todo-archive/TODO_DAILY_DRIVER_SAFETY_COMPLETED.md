# Verified Daily-Driver Safety Work

Created: 2026-10-03
Status: Archived completed increments, not overall daily-driver signoff.
Implementation baseline: `7dbf39c`; latest verified implementation: `0bf3ff6`.

These 33 checked rows were moved from the active completeness plan on
2026-10-03 without changing their completion status. They record automated and
disposable-fixture evidence, not fresh installed-build, device or distribution
acceptance. No previously unchecked test was marked passed during this cleanup.

Remaining engineering work is in the
[active TODO](../todo/TODO_DAILY_DRIVER_COMPLETENESS.md). Outstanding acceptance
is in the [daily-driver validation checklist](../operations/linux-release/daily-driver-validation-checklist.md).
Historical audits below retain their original dates, baselines and limitations;
their references to the former Priority 0 rows describe the plan at that time.

## Completed Coverage Work

- [x] Inventory existing tests and checklists; map uncovered scenarios before
  adding coverage. Reuse the [core operations release checklist](../operations/core-operations/release-checklist.md)
  and [fault-injection seams](../operations/core-operations/fault-injection-notes.md).
- [x] Add deterministic regression tests only for actual uncovered cases or
  reproduced defects; distinguish manual, automated, skipped, and blocked checks.

## Completed Safety Increments

- [x] Prevent late destination overwrite during clipboard move.
- [x] Honor cancellation before rename and before fallback source deletion.
- [x] Retain the complete destination after partial source-deletion failure.
- [x] Surface cancellation rollback failure without overwriting competing files.
- [x] Check final writeback before deleting sources in undo move fallback.
- [x] Reject special-file/replacement-symlink input in guarded copy paths.
- [x] Revalidate source identity before clipboard rename/fallback deletion.
- [x] Inject mid-stream disk-full, source-read and destination-write failures
  through local clipboard copy and forced cross-device move fallback.
- [x] Inject writeback failures through undo copy/move fallback; verify source
  preservation and explicit partial-output behavior.
- [x] Verify disappearance of open source/target paths and cancellation after
  successful finalization, using disposable fixtures only.
- [x] Reject completion to an unlinked/replaced target and preserve competing
  destination files during file-copy error cleanup. Verify both local move engines.
- [x] Track nested local-copy outputs individually; preserve untracked/replaced
  paths and completed files edited by others during failure cleanup.
- [x] Check regular-file and nested source versions before fallback deletion in
  both local move engines; reject changes and keep completed copies.
- [x] Verify local paste reconciliation, clipboard preservation, and separate
  operation/refresh errors with mocked UI tests; never retry automatically.
- [x] Carry completed-copy ownership/version receipts into paste, both merge
  branches and undo replay; refuse changed, replaced or unverifiable outputs.
- [x] Remove only registered unchanged files and empty directories during copy
  undo/rollback; preserve foreign children and replacements added after the scan.
- [x] Preflight all members of a pure-copy batch before removing any target;
  preserve unchanged peers when another target was already edited.
- [x] Preserve copied bytes for redo and failed mixed-batch undo compensation
  instead of re-reading a changed/missing original source; validate backup,
  writeback and partial-removal failures with disposable fixtures.
- [x] Protect incomplete/failed copy undo and redo with recovery markers;
  keep marked sessions across startup cleanup and a killed fixture process.
- [x] Validate copy recovery between distinct disposable filesystems for files
  and nested directories; this is not real-media or power-loss acceptance.
- [x] Run history filesystem work off the UI event loop; verify error refresh,
  separate refresh failures and repeat-request suppression with mocked UI tests.
- [x] Inspect undo-session file-content size and marked-session counts without
  changing backups, locks or markers; cap metadata scans and report partial results.
- [x] Expose read-only backup diagnostics and manual recovery guidance in Settings;
  verify loading, refresh errors, partial results and lifecycle behavior.
- [x] Refuse changed source versions during local streaming copy, including
  rewrite/truncate/append; preserve uncertain finalized outputs for inspection.
- [x] Verify the recorded pre-sync output version after writeback in both local
  copy engines; refuse edited outputs and preserve them on finalization errors.
- [x] Recheck completed fallback output receipts before source deletion and
  remove only recorded unchanged source entries, preserving late source additions.
- [x] Honor cancellation between individual fallback source removals; keep
  destination data and report partially removed source trees without automatic retry.
- [x] Retain uncertain active-file outputs on local copy read/write/writeback
  failure or cancellation; verify foreign in-place edits survive without cleanup.
- [x] Protect original overwrite backups before clipboard failure rollback;
  report blocked restoration and verify marked-session retention across cleanup.
- [x] Protect original destinations before moving them into overwrite backups;
  verify killed copy/move and merge processes, marker failures and history lifecycle.
- [x] Verify manual local-copy contents against written streams before completion;
  refuse masked output edits and retain paths on readback errors or cancellation.

## Existing Capabilities and Evidence

Do not reopen completed tracks or rebuild existing components. Browsey already
has list/grid virtualization, visible-first thumbnails, search, duplicate
scanning, copy/move conflicts, cancellation/progress, local undo, permissions,
archive passwords, shared UI controls, system themes, shortcut remapping,
default-app opening, USB formatting, MTP discovery, and rclone cloud support.

Sources for this plan:

- [README](../../README.md): supported behavior and current cloud/platform limits.
- [1.0.3 release notes](../releases/1.0.3.md): automated checks and outstanding
  fresh manual device and clean-distribution validation.
- [Local undo scope](../operations/linux-release/undo-scope.md): session-only
  history, supported operations, and unsupported cloud undo.
- [Shared UI controls](../../frontend/src/shared/ui/README.md) and
  [ModalShell](../../frontend/src/shared/ui/ModalShell.svelte): existing reuse,
  focus trapping/restoration, and keyboard behavior.
- [Thumbnail loader](../../frontend/src/features/explorer/thumbnailLoader.ts):
  existing prioritization, concurrency limits, cancellation, and cache reuse.
- [Earlier maturity strategy](../strategy/nautilus-gap-strategy.md): strategic
  context, not evidence that old findings still apply.

## Evidence Index

The following records explain what each increment verified and what it did not.
Broad operation/platform acceptance remains open in the separate checklist.

The [initial file-safety baseline](../audits/daily-driver/file-safety-baseline.md)
records the planned Linux-first target scope, current test inventory, reproduced
defects and corrections, and checks that still require devices or other platforms.
Automated fixture tests are not full manual or cross-distribution acceptance.
Remaining acceptance is tracked in the separate validation checklist; optional
expansions are product decisions, not unfinished mandatory phases.

The [local-transfer fault-injection follow-up](../audits/daily-driver/local-transfer-fault-injection.md)
records copy/move failure states, additional target-identity corrections, and
the boundaries of synthetic versus real-media evidence.

The [nested-copy and UI recovery follow-up](../audits/daily-driver/nested-copy-recovery.md)
records owned-path cleanup, source-version checks, and operation-versus-refresh
outcomes. Native installed-build acceptance remains separate.

The [copy undo ownership follow-up](../audits/daily-driver/copy-undo-ownership.md)
records completed-output receipts, conservative copy removal, pure-copy batch
preflight, and the remaining recovery and race boundaries.

The [copy recovery backup follow-up](../audits/daily-driver/copy-recovery-backups.md)
records preserved bytes for redo/compensation, protected failure sessions,
disposable cross-filesystem validation and mocked history UI recovery.

The [undo storage diagnostics follow-up](../audits/daily-driver/undo-storage-diagnostics.md)
records bounded, read-only backup measurements and the remaining retention,
manual-recovery and installed-build acceptance work.

The [copy version validation follow-up](../audits/daily-driver/copy-version-validation.md)
records changed inputs during reading, post-write output versions and the
remaining final-check/concurrent-writer boundaries.

The [active-file failure follow-up](../audits/daily-driver/active-copy-failure-retention.md)
records preserved in-place edits, partial-output retention and protected original
overwrite backups when failure rollback cannot restore an occupied destination.

The [early overwrite protection follow-up](../audits/daily-driver/early-overwrite-protection.md)
records protection before original destinations move, killed-process cleanup,
marker failures and protection lifecycle through paste/merge and history replay.

The [content verification follow-up](../audits/daily-driver/copy-content-verification.md)
records refused masked output edits, bounded open-handle readback, cancellation
and verification errors, with explicit extra-I/O cost and warm-cache measurements.

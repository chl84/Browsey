# Browsey Daily Driver Completeness Plan

Created: 2026-10-03
Status: Active; initial Priority 0 implementation and validation in progress.
Baseline: Browsey 1.0.3 and local main commit `7dbf39c`.

Goal: Make Browsey a dependable, coherent daily-driver file manager within an
explicit Linux support scope. Completeness means predictable behavior and
validated workflows, not every feature found in every other file manager.
Finish the required work below before treating optional features as release
requirements. Implementation was requested on 2026-10-03. This does not authorize
destructive tests on real devices, dependency changes, publication, or installation
without the corresponding explicit request.

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

Unchecked items are validation tasks or proposed improvements, not newly
confirmed bugs. Record a reproduction and inspect existing tests before
changing behavior. Historical audits and checked Linux 1.0 rows are not fresh
acceptance evidence for this baseline.

## Current Execution Record

The [initial file-safety baseline](../audits/daily-driver/file-safety-baseline.md)
records the planned Linux-first target scope, current test inventory, reproduced
defects and corrections, and checks that still require devices or other platforms.
Priority 0 is not complete: automated fixture tests are not full manual or
cross-distribution acceptance. Keep later phases and optional expansions open.

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

## Priority 0 Define Scope and Verify File Safety

Complete this phase first. Reliability takes precedence over feature breadth.

- [ ] Agree the daily-driver support matrix: Linux first; validate Omarchy /
  Hyprland, Fedora / GNOME Wayland, and Ubuntu LTS / GNOME Wayland. Distinguish
  tested release targets from environments that merely compile or start.
- [x] Inventory existing tests and checklists; map uncovered scenarios before
  adding coverage. Reuse the [core operations release checklist](../operations/core-operations/release-checklist.md)
  and [fault-injection seams](../operations/core-operations/fault-injection-notes.md).
- [ ] Validate copy/move/rename/trash/delete/extract with conflicts, permission
  denial, full destination, disappearing source, disconnected destination,
  symlinks, long names, Unicode, and unsupported Unix filename encodings.
  Record what is supported and what fails explicitly; never silently rename.
- [ ] Verify cancel/error outcomes across operations: report completed, skipped,
  and failed items; preserve unrelated files; retain sources until the required
  destination completion checks pass. Never automatically repeat destructive
  operations after an uncertain result.
- [ ] Test process interruption/restart with disposable fixtures. Document
  surviving partial outputs, session backup cleanup, and concurrent-instance
  ownership. Automatic resume or persistent undo requires a separate design
  review; it is not assumed safe or required by this task.
- [ ] Revalidate local undo/redo, overwrite boundaries, and the documented
  50-action/session-only limits. Do not promise recovery that is not supported.
- [x] Add deterministic regression tests only for actual uncovered cases or
  reproduced defects; distinguish manual, automated, skipped, and blocked checks.

Done when: the agreed matrix has current evidence, no unresolved data-loss or
wrong-destination defects, and clear recovery outcomes under interruption.

### Verified Safety Increments

These narrower increments make progress visible without claiming that the
broader operation/platform acceptance rows above are complete.

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
- [ ] Validate manual recovery UX, retention/space budgets and installed-build
  behavior for marked sessions. Preserved backups do not imply persistent undo,
  automatic repair/resume or atomic batches.
- [ ] Validate edits during active writes and races after per-entry checks;
  metadata snapshots are not content hashes or filesystem transactions.
- [ ] Validate installed UI partial-result refresh/recovery and real-media faults.

## Priority 1 Validate Desktop and Device Integration

- [ ] Run [native drag acceptance](../testing-native-drag.md) with Browsey and
  Nautilus: multiple files, special characters, copy/move modifiers, cancellation,
  repeated drops, scaling, and window teardown. Verify both directions.
- [ ] Validate clipboard interoperability and folder launches from other apps,
  including T3 Code. Test installed builds, not only mocked browser commands.
- [ ] Revalidate Open With and MIME defaults using text, Python, images, PDFs,
  missing handlers, failed launches, and the one-time versus default checkbox.
- [ ] Inspect desktop expectations such as revealing selected files through
  `org.freedesktop.FileManager1`. Determine current support and user demand
  before proposing a service; ordinary folder launch support already exists.
- [ ] Run [MTP acceptance](../testing-mtp.md): first connection without Nautilus,
  locked phone, reconnect, large camera folders, temporary I/O errors, copy,
  cancellation, and stale device removal.
- [ ] Run [USB acceptance](../testing-usb-format.md): mount/eject, busy volumes,
  missing optional tools, permissions after format, and correct device identity.
  Formatting tests require explicit approval and a dedicated disposable device.
- [ ] Validate network disconnect/reconnect and stale-path handling against
  controlled mounts; check that discovery and refresh settings behave as documented.
- [ ] Record fresh RPM/DEB clean install, upgrade, reinstall, and uninstall on
  the agreed distribution targets. Preserve user settings and verify the
  packaged frontend, PDFium, desktop entry, icons, and associations.

Done when: common integrations pass on installed builds, optional dependency
failures are actionable, and platform-specific limitations are explicit.

## Priority 2 Make Interaction and Feedback Consistent

- [ ] Audit selection, keyboard navigation, context menus, search transitions,
  breadcrumbs, and focus after refresh or deletion in both list and grid views.
- [ ] Audit every modal for initial focus, Tab trapping, restoration, Enter/Esc,
  safe destructive defaults, validation, busy state, and double submission.
  Reuse ModalShell and existing controls; fix deviations rather than replacing them.
- [ ] Test small windows, long filenames/messages, 100/150/200 percent display
  scaling, Cozy/Compact density, system/light/dark themes, and high contrast.
- [ ] Verify progress distinguishes byte counts, item counts, phases, and
  indeterminate work. Unknown size must not be presented as a synthetic byte total.
- [ ] Check all operation outcomes give actionable feedback, including partial
  success and refresh failures after an otherwise successful operation. Offer
  retry only where its safety and target identity are established.
- [ ] Review what multiple simultaneous operations and closing the window do.
  Add clear active-operation visibility and close guidance only where missing;
  a new queue, pause/resume engine, or history is a separate product decision.
- [ ] Verify names, labels, buttons, tooltips, disabled actions, and empty states
  use shared components and capability decisions consistently.

Done when: the same action has coherent behavior across entry points, errors do
not strand the UI, and dangerous actions cannot be triggered accidentally.

## Priority 3 Measure Performance and Accessibility

- [ ] Create reproducible disposable workloads for 10,000 and 100,000 entries,
  mixed thumbnail formats, recursive search, and controlled slow storage.
  Reuse existing thumbnail and cloud performance tests before adding harnesses.
- [ ] Measure cold/warm folder opening, first visible thumbnails, scroll/zoom,
  search, cancellation latency, memory, and disk-cache growth. Record hardware,
  data shape, OS, commit, and median/tail latency so results are comparable.
- [ ] Agree measured performance budgets; investigate the slowest path first.
  Add regression thresholds only where the environment is stable enough to
  avoid flaky CI. Do not assume that more workers improve MTP/cloud performance.
- [ ] Review application cache/undo/log retention and settings feedback for size
  and cleanup. Never delete live operations, active-session backups, or files
  still needed by open programs. The maintenance agent's caches are separate
  infrastructure, not Browsey's runtime caches.
- [ ] Perform keyboard-only and real Linux screen-reader checks for list/grid,
  selection, dialogs, errors, and progress. Labels and focus helpers already
  exist; automated accessibility checks alone do not prove full usability.
- [ ] Validate contrast, non-color selection cues, text scaling, and reduced
  motion. Implement only missing behavior and preserve existing accessibility settings.
- [ ] Decide localization scope. If approved, centralize user-facing strings
  and add English/Norwegian translations with plural, date, and size formatting
  tests. Localization is not assumed implemented or mandatory for Linux file safety.

Done when: performance has reproducible baselines, cache behavior is bounded or
explicitly documented, and important workflows pass actual accessibility checks.

## Priority 4 Make Remote Capabilities Predictable

Cloud parity is not required for a complete local file manager. Clear, safe
boundaries are required for the cloud capabilities Browsey advertises.

- [ ] Verify capability restrictions agree across menus, shortcuts, drag/drop,
  and error messages. Do not send remote paths into local-only operations.
- [ ] Verify quota/rate-limit, expired authentication, disconnection, cancellation,
  stale listings, and partial mixed-transfer outcomes with controlled remotes
  or deterministic fakes. Never test deletion against real personal cloud data.
- [ ] Keep permanent cloud deletion warnings and unsupported undo explicit;
  inspect each provider before considering trash/recycle-bin support.
- [ ] Review manual refresh, setup diagnostics, and thumbnail restrictions for
  clarity. Existing opt-in cloud thumbnails and connection tests are not new tasks.
- [ ] Decide which gaps deserve expansion: cloud Open With via managed download,
  archive operations, advanced rename, or external drag materialization. Each
  needs its own cache lifetime, conflict, cancellation, and credentials design.

Done when: remote users can predict supported actions and failure outcomes.
Keep unsupported behavior documented instead of promising blanket local parity.

## Optional Product Decisions

These are proposals, not confirmed missing release requirements. Verify current
implementation and user value before selecting any. Keep them out of the
required completion gate until explicitly approved.

- [ ] Evaluate tabs and session restoration; review state isolation, dirty
  operations, keyboard shortcuts, and memory before committing to implementation.
- [ ] Evaluate a split view for frequent copy/move workflows; define independent
  selection, focus, navigation, destination preview, and transfer ownership.
- [ ] Evaluate quick file preview and archive browsing without extraction.
  Bound memory/I/O and sandbox untrusted content; never execute previews.
- [ ] Evaluate a transfer queue or operation history. Distinguish visibility and
  scheduling from resumable transfers; not every operation can safely pause/resume.
- [ ] Decide whether to deepen Windows validation or keep maintenance-only scope.
  macOS support and new Linux package formats require separate plans.

## Verification and Completion Gate

- [ ] For every selected item, record evidence of the gap, the smallest fix,
  changed files, exact checks/results, remaining risks, and tested commit.
- [ ] Keep fixes small and independently verifiable. Reuse shared UI, error,
  task, and capability infrastructure rather than adding parallel mechanisms.
- [ ] Pass `bash scripts/maintenance/test-both.sh --strict-docs` and relevant
  security/native checks; report environmental skips without calling them passes.
- [ ] Complete fresh manual acceptance for affected device/platform workflows.
  Keep release-specific outstanding checks in the relevant release notes.
- [ ] Add an opt-in diagnostic export only if needed: version/session, safe
  dependency status, and redacted errors. Preview exported data and exclude
  credentials, archive passwords, personal paths, and communication by default.
- [ ] Update user-facing support/limitation docs and make a release through the
  existing bump, package verification, publication, and install procedure.
  Do not overwrite historical tags, release files, or validation results.
- [ ] After the required phases pass, run a maintainer-approved stabilization
  period in ordinary use and record unresolved defects before final signoff.
- [ ] Archive this plan when required work is complete; move explicitly chosen
  optional expansions into separate active plans rather than leaving it open forever.

Completeness signoff requires current evidence for the agreed support scope,
no unresolved release-blocking trust bugs, coherent and accessible interaction,
measured performance, and honest local/remote capability boundaries. It does
not require all optional product proposals or unsupported platforms.

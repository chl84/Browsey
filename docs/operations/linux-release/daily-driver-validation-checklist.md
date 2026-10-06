# Daily-Driver Validation Checklist

Created: 2026-10-03
Status: Outstanding acceptance, not a missing-feature backlog.
Source: [active daily-driver TODO](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md).
Code baseline for planning: Browsey 1.0.3 through `0bf3ff6`.

## Current Run Evidence

The [2026-10-03 Omarchy A0 run](runs/2026-10-03-a0-omarchy.md) tested candidate
`bf82750`, built/installed its production binary with maintainer approval and
completed A0-4 and A0-6 for the run's explicit fixture/process and read-only
recovery UX scopes. Other rows remain open or partial; this is not A0 or release
signoff. The run also reproduced misleading RAR capability wording, recorded as
VD-1 in the active TODO. No unrelated acceptance was inferred from test counts.

The [installed native operations follow-up](runs/2026-10-03-a0-native-operations.md)
adds ten checked subtests for move, file/folder rename, conflict rejection, trash,
permanent deletion, ZIP extraction and selected undo/redo. A0-2, A0-3 and A0-5
remain partial; their untested failure, history-limit and platform scopes are
explicit in that record. There are still 27 open top-level acceptance rows.

The [error/cancellation/concurrent-writer follow-up](runs/2026-10-03-a0-errors-and-writers.md)
adds fifteen checked subtests, including conflict policies/overwrite undo,
permissions, mid-operation cancellation and native partial-listing refresh. Two
real writers exercised active source/output changes with fixture-scoped timing
interposition. VD-1 is corrected and VD-2 extraction feedback is fixed in a tested
optimized candidate, not yet installed. The
[concurrent-writer boundary decision](../../audits/daily-driver/concurrent-writer-boundary.md)
closes the engineering review, not the remaining final-check races. A0-2, A0-3,
A0-5, A0-7 and A0-8 remain partial; the top-level count is still 27 open rows.

The [2026-10-04 Ctrl-wheel zoom follow-up](../../audits/daily-driver/ctrl-wheel-zoom.md)
records the fixes subsequently committed as `4cc722e`, before/after mock-UI
measurements and automated browser evidence. Five zoom-specific subtests under
A2-1 and A2-3 are checked below for that scope only. Installed Browsey/WebKit,
real thumbnail I/O and the remaining interaction/display cases were not
validated by this work; both parent rows and all 27 open top-level rows remain
open. Performance engineering evidence is linked separately below.

These 29 rows were moved from the former completeness plan, not automatically
marked passed. Existing automated evidence is in the
[verified-work archive](../../todo-archive/TODO_DAILY_DRIVER_SAFETY_COMPLETED.md).
Features can already work while still lacking fresh installed-build, real-device
or cross-distribution evidence.

The [2026-10-05 scoped native foundation run](runs/2026-10-05-native-foundation.md)
records separate-candidate local operation and Norwegian input acceptance, plus
scope/harness regressions. Provider results and exclusions are recorded there;
this does not close any parent row below or certify the installed build.

The [native foundation fixes and retests](runs/2026-10-05-native-foundation-fixes.md)
add generated file/tree transfer acceptance on all five providers and both
local-hub directions, including MTP and OneDrive source removal. Local undo/redo
passed a fresh retest after fixing the harness wait. The original full report
remains FAIL; its passing provider cases and the later local PASS are recorded
separately. No parent row below is closed by this bounded case coverage.

The [native startup/teardown fault verification](runs/2026-10-05-native-lifecycle.md)
records owned process exit after injected driver/app termination, timeout and
session-closure failures, plus a normal local retest. It completes NT0-4 harness
coverage; it does not accept device disconnect/reconnect or close parent rows.

The [native case/provider reporting verification](runs/2026-10-05-native-reporting.md)
adds NT0-5 setup/partial-result evidence, fresh local and local/USB passes and a
separate blocked mobile preflight. Reports preserve completed parts and unrun
cases; requirement declarations do not certify untested provider capabilities.

The [native mobile retest](runs/2026-10-05-native-mobile-retest.md) then passed all
17 local/mobile cases, including file/tree copy and move in both directions, on
clean committed code. Its report and owned teardown are PASS; the earlier mobile
preflight remains separately BLOCKED. No parent row closes from this foundation.

The [native privacy/retention verification](runs/2026-10-05-native-retention.md)
completes NT0-6 with permission/Git guards, bounded accounting and explicit
single-run cleanup verified on synthetic fixtures. Real recovery runs remain
retained; this does not close any functional acceptance row.

The [native owned-folder navigation verification](runs/2026-10-05-native-navigation.md)
completes NT1-1 with list/grid navigation on local disk, USB, network, OneDrive
and mobile, including empty folders, breadcrumbs, history, menu Refresh and F5.
It fixes stale explicit cloud refresh and verifies owned teardown/retention.
These separate-candidate results do not close the broader parent rows below.

The [native listing and recursive search verification](runs/2026-10-05-native-listing.md)
completes NT1-2 on all five providers: 75 listing/filter/search parts, owned
accessibility, independent file readback and confirmed teardown/retention passed.
Recursive OneDrive search is implemented and verified; missing-root fallback
and stale search-draft results are corrected. The broader parent rows remain open.

The [native selection verification](runs/2026-10-05-native-selection.md) completes
NT1-3 with 60 list/grid parts on all five providers and five local virtualized
selection parts using 200 generated files. Exact selected-copy membership/bytes,
owned accessibility, teardown and privacy audits passed; final reporting changes
passed a fresh local retest. This scope does not close the broader parent rows.

The [native creation verification](runs/2026-10-05-native-creation.md) completes
NT1-4 with 120 parts across all five providers, independent preservation after
every attempt, owned accessibility and confirmed teardown/privacy. Invalid leaf
names, cloud folder collisions and stale errors during corrected requests are
fixed. The broader parent rows remain open.

The [native editing, history and Properties verification](runs/2026-10-05-native-editing-history-properties.md)
completes NT1-5 through NT1-8 with 223 distinct parts: rename, mixed file
operations and Properties on all five providers, plus local history boundaries
and an owned restart. Exact generated-tree readback, owned accessibility,
confirmed teardown and fresh privacy audits passed. Failed full reports remain
retained; unchanged USB/network scopes retain their original build identity.
The broader parent rows remain open.

The [native transfer verification](runs/2026-10-06-native-transfers.md) completes
NT2-1 with 40 within-provider copy/move parts across all five targets, including
empty roots/descendants and mixed batches. Both entire generated trees/bytes,
owned accessibility, teardown and fresh privacy audit passed after fixing empty
cloud directory copies. The earlier blocked report remains retained. Conflict points and broader parent rows remain open.
NT2-2 also passed all 16 copy/move mixed-tree parts across the eight local-hub
routes, including mobile both ways. Independent trees, teardown and fresh audit
passed. NT2-3 also passed all 40 all-pairs copy/move parts on one candidate, with
selected routing receipts and independent trees across all 20 directions.
NT2-4 completed all 56 conflict cases/112 parts, including Skip, cloud directory
merge cleanup and explicit cross-kind refusals. Its corrected independent audit
and a fresh 32-part local/network integration run pass on the same app/binary;
the original audit-blocked report retains its status. Exact trees, accessibility
and owned teardown passed. NT2-5 completed all 47 unsafe-target/unique-copy parts across the five providers
on the same app/binary. A new 29-part local/cloud/mobile run and the unchanged
USB/network parts pass, with exact trees, accessibility and private teardown
audits. Its original cloud-fixture setup failure remains BLOCKED and retained.
NT2-6 passed eight representative local/USB/cloud batch cases, including genuine
local read denial, exact partial counts, remaining cut paths and combined transfer/
refresh errors. Bounded cloud-source injection tests reconciliation rather than
provider outages; network/mobile failure modes remain separate. Exact trees,
accessibility and owned teardown/private audits pass. NT2-1 through NT2-6 are
complete within their declared scopes; broader parent rows remain open.

## Run Discipline

The [scoped native acceptance suite](../../testing-native.md) provides reusable
foundation cases for local disk, USB, network, cloud and mobile. Its implementation
or policy-test success does not check any acceptance row here. Record a real
candidate run, provider outcomes and excluded watcher/device/large-file behavior
before adding acceptance evidence; unavailable configured targets are BLOCKED.

Copy this checklist into a candidate-specific run record. Record date, exact
commit/build, installed versus test binary, tester, OS/distro/compositor, device
or fixture, commands, observed outcomes and evidence links. Record each row as
PASS, FAIL, SKIP/BLOCKED, or justified N/A. Check it only after its selected scope
passes; explicitly document excluded subcases and platforms. N/A, skipped tests,
mock IPC and historical checked rows are not equivalent to a native pass.

Reuse the [core operations release checklist](../core-operations/release-checklist.md),
[Linux pre-release gate](pre-release-checklist.md) and
[Linux bugbash](bugbash-checklist.md); do not duplicate their operation semantics.
Use provider-specific [cloud checklists](../../cloud/checklists/) for real-account
quirks. Limit each release run to the affected/claimed scope, with explicit
justification for exclusions; the full daily-driver signoff needs its agreed scope.

Use disposable fixtures and controlled remotes only. Real-device formatting
requires explicit approval and a dedicated disposable device. Do not erase or
disconnect personal storage to simulate failure. Neither this checklist nor its
planning baseline authorizes installation, restart, publication or real-account
destructive testing.

## A0 File Safety and Recovery

- [ ] **A0-1** Agree the daily-driver support matrix: Linux first; validate Omarchy /
  Hyprland, Fedora / GNOME Wayland, and Ubuntu LTS / GNOME Wayland. Distinguish
  tested release targets from environments that merely compile or start.
- [ ] **A0-2** Validate copy/move/rename/trash/delete/extract with conflicts, permission
  denial, full destination, disappearing source, disconnected destination,
  symlinks, long names, Unicode, and unsupported Unix filename encodings.
  Record what is supported and what fails explicitly; never silently rename.
- [ ] **A0-3** Verify cancel/error outcomes across operations: report completed, skipped,
  and failed items; preserve unrelated files; retain sources until the required
  destination completion checks pass. Never automatically repeat destructive
  operations after an uncertain result.
- [x] **A0-4** Test process interruption/restart with disposable fixtures. Document
  surviving partial outputs, session backup cleanup, and concurrent-instance
  ownership. Automatic resume or persistent undo requires a separate design
  review; it is not assumed safe or required by this task.
  PASS for the documented Omarchy fixture/process scope in the linked run;
  power-loss/real-media acceptance remains open in A0-8.
- [ ] **A0-5** Revalidate local undo/redo, overwrite boundaries, and the documented
  50-action/session-only limits. Do not promise recovery that is not supported.
- [x] **A0-6** Validate manual recovery UX and installed-build behavior for marked
  sessions, including diagnostic accuracy, safe guidance and retained backups.
  Preserved backups do not imply persistent undo, automatic repair/resume or
  atomic batches. Storage budget/retention design is tracked in the active TODO.
  PASS for installed read-only diagnostics/guidance, restart retention and
  separately verified manual recovery in the linked run; not automatic repair.
- [ ] **A0-7** Validate edits during active writes and races after per-entry checks;
  metadata snapshots are not content hashes or filesystem transactions.
- [ ] **A0-8** Validate installed UI partial-result refresh/recovery and real-media faults.

## A1 Desktop and Device Integration

- [ ] **A1-1** Run [native drag acceptance](../../testing-native-drag.md) with Browsey and
  Nautilus: multiple files, special characters, copy/move modifiers, cancellation,
  repeated drops, scaling, and window teardown. Verify both directions.
- [ ] **A1-2** Validate clipboard interoperability and folder launches from other apps,
  including T3 Code. Test installed builds, not only mocked browser commands.
- [ ] **A1-3** Revalidate Open With and MIME defaults using text, Python, images, PDFs,
  missing handlers, failed launches, and the one-time versus default checkbox.
- [ ] **A1-4** Inspect desktop expectations such as revealing selected files through
  `org.freedesktop.FileManager1`. Determine current support and user demand
  before proposing a service; ordinary folder launch support already exists.
- [ ] **A1-5** Run [MTP acceptance](../../testing-mtp.md): first connection without Nautilus,
  locked phone, reconnect, large camera folders, temporary I/O errors, copy,
  cancellation, and stale device removal.
- [ ] **A1-6** Run [USB acceptance](../../testing-usb-format.md): mount/eject, busy volumes,
  missing optional tools, permissions after format, and correct device identity.
  Formatting tests require explicit approval and a dedicated disposable device.
- [ ] **A1-7** Validate network disconnect/reconnect and stale-path handling against
  controlled mounts; check that discovery and refresh settings behave as documented.

  Network copy implementation scope (2026-10-04): direct GIO callbacks replace
  CLI progress parsing for GVFS transfers in either direction. A separate
  cancellation watcher interrupts GIO even without callbacks; failed or
  cancelled transfers are not retried through the manual writer. Regression
  tests exercise GIO with disposable local fixtures, raw byte totals, file and
  folder aggregation, existing/racing targets, source retention, cancellation
  through the task registry and UI listener cleanup. Unknown totals remain
  indeterminate; directory metadata is not counted as file contents. These are
  candidate backend/mock-UI tests, not real SFTP, MTP or native-WebKit acceptance.
  The previously approved SFTP test folder was unavailable during this run.
  Real transfer progress and mid-transfer cancellation remain unchecked.

  Network deletion implementation scope (2026-10-04): regression tests cover
  no content download/local undo for remote deletion, supported trash,
  confirmation before unsupported trash or permanent deletion, cancellation,
  partial outcomes, symlink targets and local undo in mixed selections. An
  8 GiB sparse disposable local surrogate exercises direct GIO deletion; this
  is not a measured SFTP performance or installed-build acceptance result.
  Opt-in candidate-backend acceptance in an explicitly approved SFTP folder
  deleted a 64 MiB file and a nested directory in approximately 56 ms (one run,
  excluding fixture upload/setup). The server reported no trash support;
  unconfirmed deletion and cancellation before mutation left fixtures intact,
  confirmed deletion created no undo history, and server queries verified
  removal. GVFS retained stale positive file metadata, but a fresh FUSE directory
  listing was empty. Only uniquely allocated disposable fixtures were removed,
  including cleanup after an initial stale-metadata assertion failed.
  The read-only existing-GVFS mapping check also passed with real mounts.
  A simulated browser checked ordinary Delete routing, unsupported-trash
  confirmation with initial focus on Cancel, supported trash without a prompt,
  Shift+Delete warnings, and progress access after confirmation. These checks
  are not installed-build/native-WebKit acceptance; real supported server trash,
  disconnect/reconnect and cancellation during deletion remain unchecked.

  SFTP implementation evidence (2026-10-04): Linux GTK/GVFS authentication,
  bounded/cancellable connection handling, exact mount identity and persistent
  server-address history have Rust/frontend regression coverage. The simulated
  browser flow checks saved-server menus, reconnect and forgetting without
  disconnecting. Backend regressions also cover server-root/home/GVFS alias
  deduplication, offline history, account/port/share separation and forgetting
  saved folder aliases. Read-only GIO checks confirmed that the two already
  mounted SFTP servers map root and home URIs below the same GVFS mount; the
  corrected installed Network view has not yet been checked.
  This is not native acceptance: test the installed build against
  approved servers with distinct accounts/passwords, rejected login, Cancel,
  host-key questions, optional keyring saving, restart and stale mounts. No real
  password or host-key approval was automated.
- [ ] **A1-8** Record fresh RPM/DEB clean install, upgrade, reinstall, and uninstall on
  the agreed distribution targets. Preserve user settings and verify the
  packaged frontend, PDFium, desktop entry, icons, and associations.

## A2 Interaction and Feedback

- [ ] **A2-1** Audit selection, keyboard navigation, context menus, search transitions,
  breadcrumbs, and focus after refresh or deletion in both list and grid views.

  Automated zoom-only scope: PASS in the linked Ctrl-wheel report, not an
  installed-build acceptance pass.

  - [x] **A2-1-Z1** Preserve selection through rapid Ctrl-wheel bursts, all five
    grid sizes and list/grid round trips in the browser fixture.
  - [x] **A2-1-Z2** Preserve the visible-region anchor, including coalesced
    grid-to-list switches, and keep visible cards when zooming out at the bottom.
  - [x] **A2-1-Z3** Keep ordinary scrolling unchanged and block zoom behind
    a modal, including queued work checked by unit regressions.

  Remaining: repeat the affected zoom workflows in installed Browsey/WebKit;
  complete the parent row's keyboard, menu, search, breadcrumb and post-operation
  focus acceptance. These browser-only subtests do not close those cases.

- [ ] **A2-2** Audit every modal for initial focus, Tab trapping, restoration, Enter/Esc,
  safe destructive defaults, validation, busy state, and double submission.
  Reuse ModalShell and existing controls; fix deviations rather than replacing them.
- [ ] **A2-3** Test small windows, long filenames/messages, 100/150/200 percent display
  scaling, Cozy/Compact density, system/light/dark themes, and high contrast.

  Automated zoom-only scope: PASS in the linked Ctrl-wheel report, not a full
  density/scaling/theme matrix.

  - [x] **A2-3-Z1** Retain the requested thumbnail size when switching between
    Cozy/Compact, with the respective 8/6 px grid gaps in the browser fixture.
  - [x] **A2-3-Z2** Consume browser-native Ctrl-wheel input without page zoom
    or a device-pixel-ratio change in the tested browser profile.

  Remaining: installed Browsey/WebKit zoom and density checks, small-window and
  long-content acceptance, the 100/150/200 percent display-scaling matrix, and
  system/light/dark/high-contrast coverage. A fixed browser device ratio is not
  physical display-scaling validation.

- [ ] **A2-4** Verify progress distinguishes byte counts, item counts, phases, and
  indeterminate work. Unknown size must not be presented as a synthetic byte total.
- [ ] **A2-5** Check all operation outcomes give actionable feedback, including partial
  success and refresh failures after an otherwise successful operation. Offer
  retry only where its safety and target identity are established.
- [ ] **A2-6** Review what multiple simultaneous operations and closing the window do.
  Add clear active-operation visibility and close guidance only where missing;
  a new queue, pause/resume engine, or history is a separate product decision.
- [ ] **A2-7** Verify names, labels, buttons, tooltips, disabled actions, and empty states
  use shared components and capability decisions consistently.

## A3 Accessibility

- [ ] **A3-1** Perform keyboard-only and real Linux screen-reader checks for list/grid,
  selection, dialogs, errors, and progress. Labels and focus helpers already
  exist; automated accessibility checks alone do not prove full usability.
- [ ] **A3-2** Validate contrast, non-color selection cues, text scaling, and reduced
  motion. Implement only missing behavior and preserve existing accessibility settings.

## A4 Remote Capability Boundaries

- [ ] **A4-1** Verify capability restrictions agree across menus, shortcuts, drag/drop,
  and error messages. Do not send remote paths into local-only operations.
- [ ] **A4-2** Verify quota/rate-limit, expired authentication, disconnection, cancellation,
  stale listings, and partial mixed-transfer outcomes with controlled remotes
  or deterministic fakes. Never test deletion against real personal cloud data.
- [ ] **A4-3** Keep permanent cloud deletion warnings and unsupported undo explicit;
  inspect each provider before considering trash/recycle-bin support.
- [ ] **A4-4** Review manual refresh, setup diagnostics, and thumbnail restrictions for
  clarity. Existing opt-in cloud thumbnails and connection tests are not new tasks.

## Performance Evidence and Product Scope

Reproducible workloads, measurements and budget decisions remain engineering
work in the [active TODO](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md). Link their
results from the candidate run; measurements are not a claim that features are
missing. Support expansion, localization, tabs, split view and cloud parity are
optional decisions there, not automatically required acceptance cases.

The [Ctrl-wheel zoom report](../../audits/daily-driver/ctrl-wheel-zoom.md) extends
the active TODO's completed mock-UI workload/measurement evidence: rapid input,
anchored layout and thumbnail-resolution scheduling were corrected and measured
at 10k/100k entries. Real cold/slow-storage and first displayed native thumbnails,
agreed performance budgets and comparison with Nautilus remain unverified; no
additional performance acceptance or top-level row is marked complete.

## Findings and Signoff

A failing test becomes an implementation TODO only after a concrete reproduction,
scope and expected behavior are documented. Keep its evidence link in the run
record; do not mark a broad row complete because one fixture passed. Record
completed/skipped/failed checks separately, including environmental omissions
such as unavailable Semgrep or native GUI sessions.

Before signoff, complete the selected release's automated and security checks,
resolve release-blocking trust defects, record fresh affected-scope manual
acceptance and a maintainer-approved ordinary-use stabilization period. Update
support/limitation docs and release-specific notes through the existing release
procedure only when separately authorized. Preserved backups do not imply
persistent undo, and filesystem checks do not provide atomic batch transactions.

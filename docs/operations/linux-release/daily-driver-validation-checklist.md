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

These 29 unchecked rows were moved from the former completeness plan, not
marked passed. Existing automated evidence is in the
[verified-work archive](../../todo-archive/TODO_DAILY_DRIVER_SAFETY_COMPLETED.md).
Features can already work while still lacking fresh installed-build, real-device
or cross-distribution evidence.

## Run Discipline

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
- [ ] **A1-8** Record fresh RPM/DEB clean install, upgrade, reinstall, and uninstall on
  the agreed distribution targets. Preserve user settings and verify the
  packaged frontend, PDFium, desktop entry, icons, and associations.

## A2 Interaction and Feedback

- [ ] **A2-1** Audit selection, keyboard navigation, context menus, search transitions,
  breadcrumbs, and focus after refresh or deletion in both list and grid views.
- [ ] **A2-2** Audit every modal for initial focus, Tab trapping, restoration, Enter/Esc,
  safe destructive defaults, validation, busy state, and double submission.
  Reuse ModalShell and existing controls; fix deviations rather than replacing them.
- [ ] **A2-3** Test small windows, long filenames/messages, 100/150/200 percent display
  scaling, Cozy/Compact density, system/light/dark themes, and high contrast.
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

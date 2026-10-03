# Browsey Daily-Driver Improvement TODO

Created: 2026-10-03
Reorganized: 2026-10-03
Status: Active engineering follow-up; acceptance is tracked separately.
Baseline: Browsey 1.0.3, verified implementation through `0bf3ff6`.

Goal: Improve dependable daily use within an explicit Linux support scope.
This list contains five engineering follow-ups, not a count of missing file-manager
features. Test execution, release procedures and unapproved product proposals
are not implementation TODOs.

## Where Progress Is Tracked

- [Verified work archive](../todo-archive/TODO_DAILY_DRIVER_SAFETY_COMPLETED.md):
  33 already-checked coverage/safety increments and their audit evidence.
- [Daily-driver validation checklist](../operations/linux-release/daily-driver-validation-checklist.md):
  outstanding installed-build, platform, device, UI, accessibility and remote checks.
  An unchecked test means unverified, not unimplemented or a confirmed bug.
- [README](../../README.md) and [1.0.3 release notes](../releases/1.0.3.md):
  existing capabilities, published scope and release-specific limitations.

Browsey already implements copy/move, undo, archive passwords, shared controls,
themes, thumbnails, Open With, USB formatting, MTP discovery and rclone support.
Inspect current code and evidence before adding or replacing behavior.

## Priority 0 Safety and Recovery

- [ ] Review the remaining concurrent-writer/final-check windows and document
  the smallest justified mitigation or explicit supported boundary. Use the
  [content-verification audit](../audits/daily-driver/copy-content-verification.md)
  and existing deterministic tests; readback and metadata receipts do not make
  trees transactional. Do not assume staging, persistent undo or automatic
  resume is an approved solution. Acceptance of observed outcomes stays in
  checklist A0.
- [ ] Measure representative backup sizes and define a safe recovery-storage
  budget/retention policy before implementing any missing controls. The
  [storage audit](../audits/daily-driver/undo-storage-diagnostics.md) and
  [undo scope](../operations/linux-release/undo-scope.md) document the concrete
  gap: read-only diagnostics and a 50-action history cap are not a byte quota.
  Review existing cache/log limits and settings feedback rather than adding
  duplicate mechanisms. Never automatically purge protected recovery sessions,
  live operations or files still needed by open programs. Keep maintenance-agent
  caches separate; installed recovery UX is checklist A0.

## Priority 1 Reproducible Performance Work

- [ ] Create reproducible disposable workloads for 10,000 and 100,000 entries,
  mixed thumbnail formats, recursive search, and controlled slow storage.
  Reuse existing thumbnail and cloud performance tests before adding harnesses.
- [ ] Measure cold/warm folder opening, first visible thumbnails, scroll/zoom,
  search, cancellation latency, memory, and disk-cache growth. Record hardware,
  data shape, OS, commit, and median/tail latency so results are comparable.
- [ ] Agree measured performance budgets; investigate the slowest path first.
  Add regression thresholds only where the environment is stable enough to
  avoid flaky CI. Do not assume that more workers improve MTP/cloud performance.

These measurements also cover the extra output read pass introduced by local
copy content verification. The existing warm-tmpfs result is evidence for that
workload only, not a cold-storage or USB/MTP performance budget.

## Optional Decisions, Not Required Work

These proposals remain unapproved and have no completion checkboxes. Confirm
current support and user value before selecting an implementation. Cloud parity,
localization and additional platforms are not prerequisites for local file safety.

- Decide localization scope. If approved, centralize user-facing strings
  and add English/Norwegian translations with plural, date, and size formatting
  tests. Localization is not assumed implemented or mandatory for Linux file safety.
- Decide which gaps deserve expansion: cloud Open With via managed download,
  archive operations, advanced rename, or external drag materialization. Each
  needs its own cache lifetime, conflict, cancellation, and credentials design.
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
also an explicit engineering follow-up. Thus there are five active TODO boxes,
not five newly discovered defects. No unchecked requirement was silently dropped
or declared verified.

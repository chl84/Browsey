# Browsey Project Docs

This directory stores project documents (strategy, operations, audits, quality,
cloud notes, and TODO tracking).

The docs web app lives in `../docs-site/` and is built/deployed separately.

## Structure

- `releases/`: version-specific release notes, artifact scope, and validation evidence
- `strategy/`: product and positioning assessments
- `operations/core-operations/`: core-operations matrix, checklist, and policy
- `operations/linux-release/`: Linux release bar and Linux-specific release rules
- `audits/daily-driver/`: dated implementation/verification evidence and safety limits
- `audits/core-operations/`: gap audits tied to core-operations hardening
- `audits/linux-release/`: historical Linux 1.0 workflow reviews and release-gap audits
- `cloud/checklists/`: provider/runtime-specific cloud checklists
- `quality/`: quality baselines and engineering quality notes
- `todo/`: active TODO documents
- `todo-archive/`: completed or archived TODO documents

## Conventions

- Prefer stable, domain-based folders over date-based dump files.
- Keep active TODOs in `todo/` and move completed tracks to `todo-archive/`.
- Keep behavior definitions in operations docs; audits/checklists should refer to
  those definitions, not duplicate them.

## Current Work vs. Historical Evidence

The active [daily-driver completeness plan](todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
contains engineering follow-ups, not a checklist of missing features. Outstanding
test requirements live in the separate
[daily-driver validation checklist](operations/linux-release/daily-driver-validation-checklist.md).
The 33 completed coverage/safety increments and their evidence are in the
[verified-work archive](todo-archive/TODO_DAILY_DRIVER_SAFETY_COMPLETED.md).
Unverified acceptance is not a confirmed defect or an unimplemented feature;
reproduce a gap before adding implementation work. Optional product proposals
and release procedures are not counted as development TODOs.

The [Linux 1.0 production-readiness track](todo-archive/TODO_PRODUCTION_READY_LINUX.md)
was completed for `v1.0.0`. Its audits are dated snapshots, including gaps that
were reported before final signoff; they are not the current issue backlog.
The [RC log](operations/linux-release/release-candidate-log.md) records that
track's completion.

Operational safety rules and reusable checklists remain useful, but checked
rows from an older run do not certify a newer build. For version-specific
results and outstanding validation, consult [release notes](releases/), such as
[1.0.3](releases/1.0.3.md). Reproduce old audit findings against current code
before treating them as new work.

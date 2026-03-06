# Browsey Project Docs

This directory stores project documents (strategy, operations, audits, quality,
cloud notes, and TODO tracking).

The docs web app lives in `../docs-site/` and is built/deployed separately.

## Structure

- `strategy/`: product and positioning assessments
- `operations/core-operations/`: core-operations matrix, checklist, and policy
- `operations/linux-release/`: Linux release bar and Linux-specific release rules
- `audits/core-operations/`: gap audits tied to core-operations hardening
- `audits/linux-release/`: Linux 1.0 workflow reviews and release-gap audits
- `cloud/checklists/`: provider/runtime-specific cloud checklists
- `quality/`: quality baselines and engineering quality notes
- `todo/`: active TODO documents
- `todo-archive/`: completed or archived TODO documents

## Conventions

- Prefer stable, domain-based folders over date-based dump files.
- Keep active TODOs in `todo/` and move completed tracks to `todo-archive/`.
- Keep behavior definitions in operations docs; audits/checklists should refer to
  those definitions, not duplicate them.

# Linux 1.0 Release Candidate Log

Created: 2026-03-07
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Status: Active run log for the Linux 1.0 stabilization finish.

## Purpose

Provide one place to record RC1+ evidence for the last open Linux 1.0
stabilization items:

- bugfix-only RC runs
- provider-specific cloud acceptance coverage
- any explicit re-approval for non-bugfix changes after RC1

This log does not replace:

- `docs/operations/linux-release/pre-release-checklist.md`
- `docs/operations/core-operations/release-checklist.md`
- `docs/operations/core-operations/release-blocking-policy.md`

## How To Use This Log

For each release candidate:

1. Record the candidate metadata below.
2. Link the completed pre-release checklist run.
3. Record whether the run stayed bugfix-only after RC1.
4. Record any cloud provider acceptance coverage exercised in that RC.
5. Classify any findings using the existing release-blocking policy language.

## RC Entries

### RC1

- Candidate version/build: `1.0.0-rc1`
- Commit SHA: `50dd5cd`
- Tag/branch: `linux-production-ready`
- Date: `2026-03-07`
- Tester/reviewer: maintainer signoff
- Linked pre-release checklist: `docs/operations/linux-release/pre-release-checklist.md`
- Bugfix-only after RC1 applies from this point forward: `yes`
- Cloud provider acceptance exercised:
  - OneDrive: provider appendix completed
  - Google Drive: provider appendix completed
  - Nextcloud: provider appendix completed
- Findings summary:
  - `Release-Blocking Trust Bug`: none
  - `Acceptable Known Limitation`: documented cloud scope boundaries only
  - `Follow-Up Issue (Non-Blocking)`: none
- Notes: baseline RC after Linux stabilization track reached green automation, manual smoke, Linux bugbash, and maintainer real-use validation.

### RC2

- Candidate version/build: `1.0.0-rc2`
- Commit SHA: release-tagged `v1.0.0` commit
- Tag/branch: `linux-production-ready` -> `v1.0.0`
- Date: `2026-03-07`
- Tester/reviewer: maintainer signoff
- Linked pre-release checklist: `docs/operations/linux-release/pre-release-checklist.md`
- Bugfix-only since RC1 maintained: `yes`
- Any explicit non-bugfix re-approval since RC1: release documentation/version bump approved for final signoff
- Cloud provider acceptance exercised:
  - OneDrive: provider appendix completed and linked for Linux 1.0 signoff
  - Google Drive: provider appendix completed and linked for Linux 1.0 signoff
  - Nextcloud: provider appendix completed and linked for Linux 1.0 signoff
- Findings summary:
  - `Release-Blocking Trust Bug`: none
  - `Acceptable Known Limitation`: documented Linux/cloud scope boundaries only
  - `Follow-Up Issue (Non-Blocking)`: none
- Notes: final signoff RC with green full maintenance suite, completed provider appendices, and no non-bugfix merges after RC1 without explicit re-approval.

### Additional RCs

Duplicate the RC2 block as needed if more than two candidates are required.

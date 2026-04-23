# Linux 1.0 Pre-Release Checklist

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Status: Active Linux 1.0 release gate.

## Purpose

Define the minimum required gates before a Browsey release may claim
`production-ready for Linux`.

This checklist extends the existing core operations release documents rather
than replacing them:

- `docs/operations/core-operations/release-checklist.md`
- `docs/operations/core-operations/release-blocking-policy.md`
- `docs/operations/linux-release/release-bar.md`

## Run Metadata

- [x] Candidate version/build: `1.0.0`
- [x] Commit SHA: release-tagged `v1.0.0` commit
- [x] Release branch/tag: `linux-production-ready` -> `v1.0.0`
- [x] Tester / reviewer: maintainer signoff
- [x] Target environments covered: Fedora Workstation (GNOME Wayland), Ubuntu LTS (GNOME Wayland)
- [x] Notes link (logs, screenshots, issues): `docs/operations/linux-release/release-candidate-log.md`

## Blocking Gates

The following gates are mandatory before Linux release signoff:

- [x] `./scripts/maintenance/test-backend.sh` passes
- [x] `./scripts/maintenance/test-frontend.sh` passes
      - includes explicit blocking on unhandled runtime
        `error`/`unhandledrejection` events in Vitest and Playwright smoke
- [x] docs consistency passes in blocking mode:
      `./scripts/maintenance/test-both.sh --strict-docs`
- [x] no new or modified Linux-critical error-handling path bypasses the
      Browsey error API; validate through:
      - `scripts/maintenance/check-backend-error-hardening-guard.sh`
      - `.semgrep/typed-errors-blocking.yml`
      - `docs/ERROR_HARDENING_EXCEPTION_POLICY.md`
- [x] manual Linux smoke run is completed using
      `docs/operations/core-operations/release-checklist.md`
- [x] Linux-specific smoke additions from
      `docs/operations/linux-release/bugbash-checklist.md`
      are completed for the touched scope
- [x] no open `Release-Blocking Trust Bug` remains for the touched scope

## Packaging Artifact Review

Before release signoff, explicitly review the packaged Linux artifacts:

- [x] package/install artifact exists for the supported Fedora target
- [x] package/install artifact exists for the supported Debian/Ubuntu target
- [x] artifact review confirms app launch without dev-only prerequisites
- [x] artifact review confirms desktop entry/icon behavior matches the release
      intent
- [x] artifact review notes are linked in run metadata

## Release-Candidate Discipline

Linux 1.0 releases require at least one explicit release-candidate phase before
final signoff.

- [x] RC1 is cut before final release signoff
- [x] after RC1, merges are bugfix-only by default
- [x] any non-bugfix merge after RC1 is explicitly re-approved and documented
- [x] blocking/follow-up findings from RC runs are linked in run notes

## Notes

- If cloud is exercised, use the Linux 1.0 cloud scope from
  `docs/operations/linux-release/release-bar.md` and the provider appendices in
  `docs/cloud/checklists/`:
  - `onedrive-rclone-v1-manual-checklist.md`
  - `google-drive-rclone-manual-checklist.md`
  - `nextcloud-rclone-manual-checklist.md`
- This checklist defines release gates. It does not replace scenario semantics
  already defined in the core operations matrix/checklist.

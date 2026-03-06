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

- [ ] Candidate version/build:
- [ ] Commit SHA:
- [ ] Release branch/tag:
- [ ] Tester / reviewer:
- [ ] Target environments covered:
- [ ] Notes link (logs, screenshots, issues):

## Blocking Gates

The following gates are mandatory before Linux release signoff:

- [ ] `./scripts/maintenance/test-backend.sh` passes
- [ ] `./scripts/maintenance/test-frontend.sh` passes
- [ ] docs consistency passes in blocking mode:
      `./scripts/maintenance/test-both.sh --strict-docs`
- [ ] manual Linux smoke run is completed using
      `docs/operations/core-operations/release-checklist.md`
- [ ] Linux-specific smoke additions from
      `docs/operations/linux-release/bugbash-checklist.md`
      are completed for the touched scope
- [ ] no open `Release-Blocking Trust Bug` remains for the touched scope

## Packaging Artifact Review

Before release signoff, explicitly review the packaged Linux artifacts:

- [ ] package/install artifact exists for the supported Fedora target
- [ ] package/install artifact exists for the supported Debian/Ubuntu target
- [ ] artifact review confirms app launch without dev-only prerequisites
- [ ] artifact review confirms desktop entry/icon behavior matches the release
      intent
- [ ] artifact review notes are linked in run metadata

## Release-Candidate Discipline

Linux 1.0 releases require at least one explicit release-candidate phase before
final signoff.

- [ ] RC1 is cut before final release signoff
- [ ] after RC1, merges are bugfix-only by default
- [ ] any non-bugfix merge after RC1 is explicitly re-approved and documented
- [ ] blocking/follow-up findings from RC runs are linked in run notes

## Notes

- If cloud is exercised, use the Linux 1.0 cloud scope from
  `docs/operations/linux-release/release-bar.md` and the controlled QA/setup
  guidance already documented for cloud.
- This checklist defines release gates. It does not replace scenario semantics
  already defined in the core operations matrix/checklist.

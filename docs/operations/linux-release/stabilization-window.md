# Linux 1.0 Stabilization Window

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Status: Active Linux 1.0 release-discipline policy.

## Purpose

Define when the Linux 1.0 stabilization window starts, when it ends, and which
merge/release-candidate rules apply while the window is active.

This document complements:

- `docs/operations/linux-release/release-bar.md`
- `docs/operations/linux-release/pre-release-checklist.md`
- `docs/operations/core-operations/release-checklist.md`

## Start Criteria

The Linux 1.0 stabilization window starts when all of the following are true:

- the Linux 1.0 release bar is defined
- pre-release gates are documented
- packaging targets for the supported Linux surface are decided
- docs and bug-report intake exist for Linux 1.0 work
- release work shifts from feature expansion to hardening, validation, and
  bugfixes

## End Criteria

The stabilization window ends only when one of these happens:

1. Linux 1.0 release signoff is granted after the pre-release checklist and
   exit criteria are satisfied.
2. The window is explicitly reset because release-blocking findings or scope
   changes require returning to broader feature work.

## Release-Candidate Discipline

- RC1 must be cut before final Linux 1.0 release signoff.
- After RC1, merges are bugfix-only by default.
- Any non-bugfix merge after RC1 requires explicit re-approval and a documented
  reason in release notes or run notes.
- RC findings must be classified using the existing core operations
  release-blocking language:
  - `Release-Blocking Trust Bug`
  - `Acceptable Known Limitation`
  - `Follow-Up Issue (Non-Blocking)`

## What This Policy Does Not Claim

This document defines the stabilization window and merge discipline only. It
does not claim that:

- RC runs have already happened
- Linux install/upgrade validation is complete
- the Linux 1.0 exit criteria have already been met

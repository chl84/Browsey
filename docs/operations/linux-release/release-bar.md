# Linux 1.0 Release Bar

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Status: Active source of truth for the Browsey Linux 1.0 production claim.

## Purpose

Define what `production-ready for Linux` means for Browsey, which Linux target
surface is covered by that claim, which features must be stable before the
claim is made, and how release-blocking decisions are made.

This document builds on existing release operations documents rather than
replacing them:

- `docs/operations/core-operations/release-blocking-policy.md`
- `docs/operations/core-operations/release-checklist.md`

## Production-Ready for Linux

Browsey may be described as `production-ready for Linux` only when all of the
following are true:

1. Local trust-critical file operations are validated through the core
   operations release checklist and pass without open Linux release-blocking
   trust bugs.
2. The supported Linux target surface below has been validated on release
   candidates.
3. Packaging/install/upgrade behavior is verified on the supported Linux target
   surface.
4. User-facing error handling for Linux-critical flows uses the Browsey error
   API rather than ad-hoc/local error formats.
5. Known limitations are documented clearly enough that users can distinguish
   supported behavior from preview/beta behavior.

Linux 1.0 enforces this through existing engineering controls rather than
case-by-case release memory:

- `scripts/maintenance/check-backend-error-hardening-guard.sh`
- `.semgrep/typed-errors-blocking.yml`
- `docs/ERROR_HARDENING_EXCEPTION_POLICY.md`

`Production-ready for Linux` does not mean every optional integration or every
desktop environment is equally supported. It means the validated Linux target
surface and the feature set below are stable enough for normal end-user use
without requiring workaround-heavy operation.

## Supported Linux 1.0 Target Surface

Linux 1.0 support is scoped to this target surface:

- Fedora Workstation: primary release validation target
- Ubuntu LTS desktop: required Debian-family validation target
- GNOME Wayland: primary desktop/session target

Other Linux distributions or desktop environments may work, but are outside the
main Linux 1.0 production claim unless explicitly added later.

## Features That Must Be Stable Before Linux 1.0

The Linux 1.0 production claim requires stability for these features:

- browsing local directories and network-visible locations
- opening files and folders
- copy and move for local filesystem paths
- rename and advanced rename for local filesystem paths
- trash, restore, purge, and permanent delete for local filesystem paths
- archive extraction and compression for supported formats
- search
- properties and permissions editing on supported Linux paths
- settings persistence, restore defaults, and shortcut persistence
- packaging/install/upgrade/uninstall behavior on the supported Linux target
  surface

These areas may remain available but are not part of the main Linux 1.0
production claim:

- cloud features (`rclone`-backed remotes)
- provider-specific cloud behavior beyond documented limited support
- desktop environments outside the validated target surface

## Cloud Scope Decision for Linux 1.0

For the Linux 1.0 production claim, cloud is outside the main claim.

Cloud remains:

- opt-in
- limited in feature scope
- documented separately from the local/core Linux 1.0 guarantee

Cloud regressions still matter, but they do not by themselves invalidate the
main Linux 1.0 production claim unless they break local browsing, settings, or
other non-cloud Browsey behavior.

## Release-Blocking Decision Flow

Linux release-blocking decisions must use the same language and base policy as
the core operations hardening documents.

Base release-blocking language:

- `Release-Blocking Trust Bug`
- `Acceptable Known Limitation`
- `Follow-Up Issue (Non-Blocking)`

Source of truth for trust-critical scenario blocking:

- `docs/operations/core-operations/release-blocking-policy.md`
- `docs/operations/core-operations/release-checklist.md`

Linux 1.0 work may add Linux-specific validation rows or packaging checks, but
it must not create a parallel severity vocabulary that competes with the core
operations release-blocking policy.

## Stabilization Mode for the Linux 1.0 Track

While the Linux 1.0 track is active, Browsey should be treated as being in
Linux stabilization mode.

That means:

- bugfixes, hardening, validation, packaging, docs, and release-gating work are
  prioritized over new feature work
- feature work that changes Linux 1.0 scope requires explicit re-approval
- release-candidate branches/tags should be evaluated against the existing core
  operations release checklist plus any Linux 1.0 additions

This is a release-discipline rule for the Linux 1.0 track, not a blanket ban on
all future feature development.

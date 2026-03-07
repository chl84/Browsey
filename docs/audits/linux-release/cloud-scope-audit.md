# Linux Cloud Scope Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 5 cloud scope, provider support, and opt-in safety for the Linux 1.0 claim.

## Purpose

Capture the concrete evidence that supported cloud providers are inside the main
Linux 1.0 production claim while disabled cloud still keeps local/network
browsing isolated from `rclone` setup concerns.

## Evidence Reviewed

- `docs/operations/linux-release/release-bar.md`
- `README.md`
- `docs-site/src/content/pages.ts`
- `frontend/src/features/explorer/state.ts`
- `frontend/src/features/explorer/state.test.ts`
- `frontend/src/features/settings/cloudSetup.test.ts`

## Confirmed Findings

### Cloud scope is explicitly decided for Linux 1.0

The Linux 1.0 release bar already makes the product decision explicit:

- supported cloud providers are inside the main Linux 1.0 production claim
- cloud remains opt-in
- cloud remains provider-scoped and feature-scoped
- cloud setup requirements and boundaries are documented separately from the
  local/core guarantee

This decision is also reflected in user-facing docs:

- `README.md`
- docs site cloud sections and Linux-specific docs pages

### Disabled cloud does not interfere with Network/local browsing

Explorer state currently enforces cloud isolation when `cloudEnabled` is false:

- `loadNetwork()` filters out `rclone://` entries
- cloud setup probing is skipped entirely
- no cloud onboarding notice is shown

Test coverage exists for this exact behavior in
`frontend/src/features/explorer/state.test.ts`:

- `does not show cloud entries or probe cloud setup when cloud is disabled`

## What This Audit Does Not Claim

This audit does not by itself prove:

- every supported provider has passed controlled QA remote validation
- every optional cloud feature belongs inside the Linux 1.0 release-blocking
  matrix

Those remain separate Step 5 decisions/items.

## Conclusion

This audit supports checking off:

- `Ensure cloud never interferes with local file browsing when disabled`
- `Make an explicit 1.0 decision for cloud scope`
  - specifically the branch where supported cloud providers are part of the
    main Linux 1.0 production claim

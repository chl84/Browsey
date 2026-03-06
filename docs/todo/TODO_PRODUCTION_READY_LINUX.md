# TODO: Make Browsey Production-Ready for Linux

Goal: Move Browsey from a strong Linux-first beta into a production-ready Linux release by hardening core workflows, tightening release gates, and validating real install/use/update behavior on supported Linux targets.

## 1. Define the release bar
- [x] Define what `production-ready for Linux` means for Browsey
- [x] Lock the supported target surface for 1.0:
  - [x] Fedora as primary validation target
  - [x] at least one Debian/Ubuntu-based distro
  - [x] GNOME Wayland as primary desktop/session
- [x] Define where Linux release-blocking decisions live and how they are tracked, building on:
  - [x] `docs/operations/core-operations/release-blocking-policy.md`
  - [x] `docs/operations/core-operations/release-checklist.md`
- [x] Define which features must be stable before 1.0
- [x] Decide early whether cloud is inside or outside the main `production-ready for Linux` claim
- [x] Move the project from active feature iteration to explicit stabilization mode

## 2. Harden core file workflows
- [x] Review these flows end-to-end for regressions and edge cases:
  - [x] browse directories
  - [x] copy and move
  - [x] rename and advanced rename
  - [x] trash and permanent delete
  - [x] compress and extract
  - [x] search
  - [x] properties and permissions
  - [x] open with / open file
- [x] Create a bugbash checklist for each core workflow
- [ ] Fix every data-loss or wrong-destination bug before further polish

## 3. Tighten file operation safety
- [x] Verify destructive operations always have correct guardrails
- [ ] Verify conflict preview always matches the real operation
- [ ] Test aborted copy/move/extract flows for partial outputs and recovery
- [x] Verify undo/redo boundaries and document the actual supported scope
- [ ] Ensure errors never leave the UI in an unknown state without a clear recovery path

## 4. Harden Linux-specific behavior
- [ ] Test mounts, eject, and removable media against real Linux setups
- [ ] Test SMB/NFS/GVFS scenarios on GNOME Wayland
- [ ] Verify clipboard integration on GNOME Wayland with and without `xclip`
- [ ] Test terminal launch, permissions editing, and trash behavior on common Linux environments
- [x] Confirm Browsey behaves correctly when optional dependencies are missing:
  - [x] `ffmpeg`
  - [x] `rclone`
  - [x] `xclip`

## 5. Keep cloud opt-in but safe
- [ ] Ensure cloud never interferes with local file browsing when disabled
- [x] Harden all empty/error/unsupported states in cloud onboarding
- [ ] Validate supported cloud providers against controlled QA remotes or equivalent reproducible test setups:
  - [ ] OneDrive
  - [ ] Google Drive
  - [ ] Nextcloud
- [x] Ensure cloud limitations are explicit in both UI and docs
- [x] Ensure failed `rclone` setup never feels like a general Browsey failure
- [ ] Make an explicit 1.0 decision for cloud scope:
  - [ ] cloud remains clearly marked as limited/beta and out of the main production claim
  - [ ] or cloud enters the Linux 1.0 support matrix with provider-specific acceptance coverage

## 6. Stabilize settings and persistence
- [ ] Test all settings for roundtrip and restore-defaults behavior
- [ ] Confirm settings changes do not require restart unless explicitly documented
- [x] Remove or clarify settings that still feel internal or under-explained
- [ ] Verify persisted settings migration across app versions

## 7. Increase test coverage where it matters
- [ ] Add regression tests for every stabilization bug that gets fixed
- [ ] Increase coverage around hooks/state seams that break under small changes
- [ ] Add more Linux-focused e2e smoke coverage for core workflows
- [x] Reuse the existing core-operations release checklist as the base Linux manual smoke gate, and extend it only where Linux 1.0 coverage still has gaps
- [x] Define any additional Linux 1.0 smoke rows outside core operations, for example:
  - [x] app start
  - [x] browse/open directory
  - [x] search
  - [x] settings open/apply/restore defaults
- [ ] Turn important production risks into explicit blocking checks in maintenance scripts:
  - [ ] fail on unhandled test errors/rejections
  - [ ] fail when required release-checklist coverage is missing
  - [x] keep advisory checks separate from release-blocking checks

## 8. Add release gating
- [x] Define a mandatory pre-release checklist
- [x] Require the following before Linux release:
  - [x] `./scripts/maintenance/test-backend.sh`
  - [x] `./scripts/maintenance/test-frontend.sh`
  - [x] docs consistency checks
  - [x] manual Linux smoke run based on `docs/operations/core-operations/release-checklist.md`
- [x] Require explicit review of packaging artifacts before release
- [x] Add a release-candidate phase before final release
- [x] Define release-candidate merge policy:
  - [x] bugfixes only after RC1
  - [x] no feature merges without explicit re-approval

## 9. Harden packaging and installation
- [ ] Test RPM install, upgrade, and reinstall on clean Fedora
- [ ] Define and validate the install path for the Debian/Ubuntu target surface:
  - [ ] decide package/distribution format
  - [ ] test install and upgrade on a clean Debian/Ubuntu-based environment
  - [ ] document uninstall behavior on that environment
- [ ] Decide whether downgrade is supported:
  - [ ] if yes, test downgrade explicitly
  - [ ] if no, document downgrade as unsupported
- [ ] Confirm the installed app starts without dev-only prerequisites
- [ ] Verify icons, desktop entry, file associations, and permissions in installed builds
- [ ] Test app data/log/cache behavior across version upgrades
- [ ] Document the exact install and upgrade path for Linux users

## 10. Improve observability and supportability
- [ ] Require all new or modified error-handling code to use the Browsey error API rather than ad-hoc/local error formats
- [ ] Remove remaining stringly or one-off error seams from Linux-critical flows
- [ ] Ensure logs are useful for real support/debug cases
- [ ] Remove or reduce noisy low-value logging
- [ ] Ensure error surfaces show user-facing language rather than internal phrasing
- [ ] Define a minimal Linux bug-report template:
  - [ ] distro
  - [ ] desktop/session
  - [ ] Browsey version
  - [ ] logs
  - [ ] repro steps

## 11. Bring docs to 1.0 level
- [ ] Update README to reflect the actual Linux stability level
- [ ] Add a `Known limitations on Linux` page
- [ ] Add a recovery/troubleshooting page
- [ ] Document optional dependencies and degraded behavior without them
- [ ] Document cloud as a separate limited feature set

## 12. Finish with a real stabilization phase
- [ ] Declare a bounded stabilization window with explicit start/end criteria
- [ ] Run multiple release candidates with bugfixes only
- [ ] Collect findings from real Linux use, not just local development testing
- [ ] After RC1, allow only bugfix-only merges unless explicitly re-approved
- [ ] Do not label Browsey `production-ready for Linux` before this phase completes cleanly

## Exit Criteria
- [ ] No open Linux release-blocking trust bugs in copy/move/delete/trash/extract flows under the core-operations release-blocking policy
- [ ] Linux install/update/uninstall validated on supported target environments
- [ ] All core workflows pass both automated checks and manual Linux smoke tests
- [ ] Cloud scope is explicitly resolved for Linux 1.0 and matches both tests and documentation
- [ ] No non-bugfix merges have landed since RC1 without explicit re-approval

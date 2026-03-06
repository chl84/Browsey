# TODO: Rclone Onboarding (Settings-first)

Goal: Keep external `rclone config` as the setup model, but make Browsey clearly explain cloud setup status, missing prerequisites, and next steps from inside the app.

Completion note:
- Completed in commit `2b79fd3` (`Add rclone onboarding diagnostics and docs`).
- Settings status-copy coverage is implemented at helper/state level (`frontend/src/features/settings/cloudSetup.test.ts` and `frontend/src/features/explorer/state.test.ts`) rather than as a dedicated `AdvancedSection` render test.

User preference locked:
- guided onboarding only
- Settings first
- `Network` as a secondary prompt surface

## Backend
- [x] Add a new Tauri command: `cloud_setup_status()`
- [x] Define a structured setup-status response type with:
  - [x] `state`
  - [x] `configured_path`
  - [x] `resolved_binary_path`
  - [x] `detected_remote_count`
  - [x] `supported_remote_count`
  - [x] `unsupported_remote_count`
  - [x] `supported_remotes`
- [x] Treat `state` as a closed enum for this iteration
- [x] Add or adjust typed error codes in `rclone_path` so setup state does not depend on parsing message text
- [x] Split `InvalidConfig` into at least:
  - [x] `binary_missing`
  - [x] `invalid_binary_path`
- [x] Add a distinct setup state for `rclone` present but not usable:
  - [x] unsupported or too-old `rclone` version
  - [x] failed runtime readiness validation
- [x] Map DB/config read failures into a setup-status state such as `config_read_failed`
- [x] Implement the setup inspection flow:
  - [x] load saved `rclonePath`
  - [x] resolve effective binary path
  - [x] validate runtime/version readiness
  - [x] fetch remotes from `rclone`
  - [x] classify supported vs unsupported remotes
- [x] Keep `list_cloud_remotes_sync_best_effort()` unchanged for `Network` best-effort behavior

## Frontend Data/Service Layer
- [x] Add `CloudSetupStatus` TypeScript type in the cloud/network frontend service layer
- [x] Add frontend service call for `cloud_setup_status`
- [x] Ensure status refresh can be triggered on Settings open and after `Rclone path` edits
- [x] Lock refresh policy for `Rclone path` edits:
  - [x] do not probe on every keystroke
  - [x] refresh on blur with a short debounce
  - [x] keep the currently displayed status while a new probe is in flight

## Settings UI
- [x] Extend `Settings > Advanced` with a cloud/rclone status block
- [x] Place the status block above the existing `Rclone path` field
- [x] Show a state-specific headline and next-step text
- [x] Show resolved binary path when available
- [x] Show supported remote count
- [x] Show supported remote labels when setup is `ready`
- [x] Keep `Rclone path` as the only editable cloud setup field in-app for this iteration
- [x] Refresh the status block when:
  - [x] the settings modal opens
  - [x] `Rclone path` changes

## Network UI
- [x] Call `cloud_setup_status` separately for onboarding/status decisions; do not infer onboarding state from `list_cloud_remotes_sync_best_effort()`
- [x] Add a compact onboarding hint in `Network` when:
  - [x] no cloud remotes are shown
  - [x] cloud setup is not `ready`
  - [x] cloud setup is not in a transient unknown/error-suppressed state
- [x] Keep the `Network` hint brief and point users to Settings
- [x] Do not add a separate cloud wizard/modal in this iteration

## Copy and Docs
- [x] Update README/docs to say Browsey supports:
  - [x] autodetect of `rclone`
  - [x] explicit `Rclone path`
  - [x] external setup via `rclone config`
  - [x] in-app status/diagnostics in Settings
- [x] Keep supported provider scope explicit: OneDrive, Google Drive, Nextcloud

## Tests
- [x] Backend test: returns `binary_missing` when autodetect fails
- [x] Backend test: returns `invalid_binary_path` for bad explicit path
- [x] Backend test: returns the distinct unusable-runtime state for unsupported or too-old `rclone`
- [x] Backend test: returns `no_supported_remotes` when `rclone` works but no supported remotes exist
- [x] Backend test: returns `ready` with counts/remotes when supported remotes exist
- [x] Backend test: unsupported remotes affect counts but are not surfaced as supported
- [x] Frontend test: Settings Advanced renders correct state block for each status
- [x] Frontend test: changing `Rclone path` does not probe on every keystroke
- [x] Frontend test: `Rclone path` blur triggers a debounced status refresh
- [x] Frontend test: `Network` shows setup hint only when setup is not ready and no cloud remotes exist
- [x] Frontend test: `Network` does not show the setup hint for transient/error-suppressed cloud discovery failures
- [x] Frontend test: existing `Network` cloud entries still render unchanged when remotes are available

## Out of Scope for This Iteration
- [x] No in-app `rclone config`
- [x] No OAuth or login wizard
- [x] No new persistent settings beyond existing `rclonePath`

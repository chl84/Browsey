# Linux Optional Dependencies Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 4 optional-dependency behavior on supported Linux targets.

## Purpose

Document which optional-dependency-missing behaviors are already underpinned by
code and tests for the Linux 1.0 track, and which areas still need broader
manual validation before the entire Step 4 item can be considered closed.

## Evidence Reviewed

- `src/commands/system_clipboard/mod.rs`
- `src/commands/system_clipboard/error.rs`
- `src/commands/thumbnails/thumbnails_video.rs`
- `frontend/src/features/explorer/context/createContextActions.ts`
- `frontend/src/features/explorer/context/createContextActions.test.ts`
- `frontend/src/features/explorer/thumbnailLoader.test.ts`
- `src/commands/cloud/setup_status.rs`
- `frontend/src/features/explorer/state.test.ts`
- `README.md`

## Status Summary

| Dependency | Current Linux 1.0 status | Basis |
|---|---|---|
| `xclip` | Verified degraded behavior | GNOME Wayland branch is tested, missing helper maps to typed clipboard-tool errors, and frontend copy/cut keep internal clipboard flow usable with explicit degraded toasts. |
| `rclone` | Verified degraded behavior | Cloud setup reports `binary_missing`, network onboarding shows guided next step when cloud is enabled, and cloud is disabled by default so local browsing is not coupled to missing `rclone`. |
| `ffmpeg` | Verified degraded behavior | Backend video-thumbnail path fails cleanly when `ffmpeg` is absent, frontend thumbnail loading keeps the visible state on icon fallback instead of breaking, and docs/UI already position video thumbs as optional. |

## Verified: Missing `xclip`

Current Linux evidence is strong enough to treat missing `xclip` as a verified
degraded-behavior case:

- GNOME Wayland clipboard routing explicitly avoids `wl-clipboard` and prefers
  `xclip` to avoid shell focus/dock side-effects.
- The GNOME Wayland detection branch now has backend unit coverage.
- Missing clipboard helpers map to the typed
  `SystemClipboardErrorCode::ClipboardToolMissing` path.
- Frontend local `copy` and `cut` flows keep working even when system clipboard
  sync fails; the user gets a degraded toast instead of a broken local action.

This is sufficient to support the Linux 1.0 claim that missing `xclip` degrades
cross-window/system clipboard interoperability, but does not break Browsey's
internal local clipboard workflows.

## Verified: Missing `rclone`

Current Linux evidence is also strong enough to treat missing `rclone` as a
verified degraded-behavior case:

- Cloud setup inspection reports `binary_missing` when autodetect fails.
- Explorer network state shows a guided onboarding notice rather than a generic
  failure when cloud is enabled but `rclone` is unavailable.
- Cloud integration is off by default, so normal local/network browsing does
  not depend on `rclone` being installed.
- When cloud is disabled, the network view does not probe cloud setup and does
  not surface cloud entries.

This is sufficient to support the Linux 1.0 claim that missing `rclone`
degrades only cloud surfaces, not the core local file-manager experience.

## Verified: Missing `ffmpeg`

Current Linux evidence is now strong enough to treat missing `ffmpeg` as a
verified degraded-behavior case:

- Video-thumbnail rendering fails cleanly when `ffmpeg` is unavailable.
- User-facing docs already state that video thumbnails are optional and fall
  back to icons when `ffmpeg` is missing.
- Frontend thumbnail loading now has explicit coverage for failed local video
  thumbnail requests, keeping the item on icon fallback rather than breaking
  visible thumbnail state.

## Conclusion

For the optional-dependency sub-items in Step 4:

- safe to check off:
  - `ffmpeg`
  - `xclip`
  - `rclone`

The parent Step 4 optional-dependencies item is now supported strongly enough
to check off for the Linux 1.0 track.

# Cloud editor writeback validation — 2026-10-08

The implementation is commit `24aa48994471d3c425ae809383ea2d9604cfd4f4` on
`feat/cloud-edit-writeback`, after published 1.0.5. The
published tag and release assets are unchanged. Validation used generated data,
private profiles and the existing rclone configuration in place. Credentials were
not copied into fixtures or reports.

## Provider conditional writes

Disposable ordinary text files on Google Drive and OneDrive rejected a deliberately
stale `If-Match` with HTTP 412, leaving original bytes unchanged. A subsequent write
with the current validator succeeded and returned the same object ID. Both unique
test roots were moved to provider trash after testing; no permanent purge was used.

The first optimized Google Drive candidate used a private XDG profile and a generated
working-copy manifest bound to an owned file ID and captured ETag. This exercises
startup recovery and the actual background watcher/save engine, independently of
frontend mocks. A second cloud file had exactly the same name and different bytes.
An external atomic replacement of the local editor copy saved to the selected ID;
the identically named neighbor remained unchanged. A subsequent change by another
cloud client caused a conflict, retained the newer cloud bytes, local edits and
upload journal, and remained stopped after restarting the normal-feature candidate.
That invocation used `cargo build --release` directly: its backend ran, but the
window tried to load the configured development URL without a running Vite server.
It therefore did not validate the packaged interface. The UI checks were repeated
using the existing release workflow's Tauri CLI build, which embeds the frontend.

The first native fixture setup attempted to obtain a directory ID from rclone's
`lsjson --stat`, which omits that ID. It stopped before launching Browsey. The exact
generated root was subsequently identified and moved to Drive trash, and the
fixture was corrected to retrieve its own folder ID directly.

## Packaged native interface

The final candidate was built with the existing production flow:
`frontend/node_modules/.bin/tauri build --ci --no-bundle -- --locked`.
No native-test feature or test IPC was enabled. Its SHA-256 is
`c1ade95a75d8d53be1d60d4082afd82073e8ceb5ba14f9168f1920b4702cbbdf`.

Native AT-SPI actions and reads were restricted to the candidate's PID. The final
run confirmed the rendered **Cloud saves: Saved** status after an atomic editor
save and independently verified the selected cloud object's bytes and stable ID.
Its same-name neighbor was unchanged. After a separate client changed the original,
the rendered **Cloud saves: 1 need attention** status agreed with a retained
conflict journal, local edits and the newer cloud version. The window's own
**Close window** action exited normally, and restarting retained the conflict
without writing over the newer cloud bytes.

Accessibility setup failures are not counted as passing UI checks. Python aborted
when the desktop AT-SPI socket refused its connection; coredump metadata located
the abort in libatspi/GLib, and available memory ruled out exhaustion. A private
session using the desktop broker could not activate its registry, and a subsequent
direct daemon initially failed to pass its address to the registry. The passing
run used a directly launched private accessibility daemon with its address set
before launch. No desktop service was restarted or reconfigured. The attempted
stop of an insufficiently identified broker process was rejected by automatic
approval review; it was left untouched and exited with its test session.

Every generated cloud fixture, including interrupted setups, was moved to normal
Drive trash using verified owned roots or IDs. The final fixture was
`browsey-writeback-native-385b2fce1a1248e095a26f9e1faf8905`. This validation did not
install the candidate or publish another release.

The generated private profiles and package extraction were removed after testing,
and the owned mock preview server was stopped. A local RPM/DEB packaging step was
outside the requested scope of this change; its generated archives were removed
after the maintainer's correction. Neither package was installed or published.

## Regression coverage

- HTTP fixtures for Google Drive, OneDrive and Nextcloud: metadata/upload races
  rejected by conditional PUT, stale versions produce no write, same-name Drive
  objects remain independent, OneDrive baseline reads use drive/item IDs, native
  Google documents and weak validators are refused.
- Durable working copies: reload/recovery after a lost successful response without
  a second write, retained conflict snapshots, edits arriving during upload,
  journal path containment, legacy manifests staying manual, fresh status after
  new manual edits and original-byte verification before enabling older copies.
- Frontend: complete byte progress remains “Saving” until confirmation, old initial
  snapshots cannot hide live conflicts, recovery actions remain available, and
  reopening the dialog retains actions for newer manual edits. The collaborative
  preview was used to inspect the badge and conflict dialog.
- Full Rust workspace/all-feature regression (870 passed, 19 ignored, plus the
  GLib regression), strict Clippy, frontend type checks,
  lint/build, 727 frontend unit tests and 108 browser workflow tests passed. Focused
  follow-up checks cover subsequent status/progress corrections.
- Native suite policy tests that inspect helper processes passed when rerun with
  process access; their first sandboxed invocation could not perform those checks.

Nextcloud coverage here uses local protocol fixtures, not a live Nextcloud server.
Native Google documents/exports and unsupported remote configurations remain manual.
OneDrive automatic replacement is limited to the conditional simple upload route
for files up to 250 MB. General local file operations were not changed; the new
watcher is scoped to working copies, ignores access events and does not poll cloud
listings or hash idle files. Copy-specific locks and short manifest locks keep
network transfers and hashing outside the registry lock and main thread.

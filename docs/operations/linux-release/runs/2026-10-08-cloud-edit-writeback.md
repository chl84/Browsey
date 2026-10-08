# Cloud editor writeback validation — 2026-10-08

The change is on `feat/cloud-edit-writeback`, after published 1.0.5. The
published tag and release assets are unchanged. Validation used generated data,
private profiles and the existing rclone configuration in place. Credentials were
not copied into fixtures or reports.

## Provider conditional writes

Disposable ordinary text files on Google Drive and OneDrive rejected a deliberately
stale `If-Match` with HTTP 412, leaving original bytes unchanged. A subsequent write
with the current validator succeeded and returned the same object ID. Both unique
test roots were moved to provider trash after testing; no permanent purge was used.

The Google Drive production candidate used a private XDG profile and a generated
working-copy manifest bound to an owned file ID and captured ETag. This exercises
startup recovery and the actual background watcher/save engine, independently of
frontend mocks. A second cloud file had exactly the same name and different bytes.
An external atomic replacement of the local editor copy saved to the selected ID;
the identically named neighbor remained unchanged. A subsequent change by another
cloud client caused a conflict, retained the newer cloud bytes, local edits and
upload journal, and remained stopped after restarting the normal candidate.

The first native fixture setup attempted to obtain a directory ID from rclone's
`lsjson --stat`, which omits that ID. It stopped before launching Browsey. The exact
generated root was subsequently identified and moved to Drive trash, and the
fixture was corrected to retrieve its own folder ID directly.

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
- Full Rust workspace/all-feature regression, strict Clippy, frontend type checks,
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

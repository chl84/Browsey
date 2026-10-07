# Google Drive object-identity implementation and acceptance

Date: 2026-10-08 (Europe/Oslo). The maintainer requested implementation of the
identical-name defect reproduced in the
[installed-build difference run](2026-10-07-google-drive-provider-differences.md),
with particular attention to avoiding general file-handling overhead.

## Build and isolation

The implementation is an uncommitted working-tree change based on
`3fab6c445ccb9ef74bd481f60d9dbe7d8cbe6794`. The
[implementation guide](../../../cloud/google-drive-object-identity.md)
records the sequence, identity format, performance boundaries and limitations.

The final candidate was built with the ordinary production command
`tauri build --no-bundle -- --locked`, without test IPC or native-test features.
Its executable is `target/release/browsey`, SHA-256:
`34b2f4bc14c8ad9c2ec9f0a7eae3efee547d42ae8b3d4ab79d2c6663b9c4fa15`.
The installed 1.0.4 executable was not replaced and retains SHA-256:
`244152d699e252bf2425152a879f0080a5f19f240c70fb8c2dfcd27127110622`.

Native WebKitGTK/AT-SPI actions and window-bound input targeted only the owned
candidate process and private XDG profile. All cloud writes were inside one
generated, ownership-marked Google Drive root. Independent rclone 1.75.1 and
scoped Drive metadata queries checked IDs, sizes, checksums and exact bytes.
The existing OAuth desktop remote was used in place; no credentials were
printed, copied into fixtures or committed. This is bounded candidate
acceptance, not a completed release or certification of every Google remote
authentication mode.

## Native results

The initial candidate SHA-256 was
`1252181db475e1e3d8670ffdd569051daa3b49bcb1ef8ddfe4703550aadf0d80`.
The two phases below distinguish initial identity checks from the final build.

| Case | Evidence and result |
| --- | --- |
| Identical-name file listing | **PASS, both candidates:** two genuine `same.txt` objects appear as separate 24-byte and 47-byte rows, including cold startup of the final candidate |
| Selected file download | **PASS, initial:** each row copied separately to a local directory produced that object's exact bytes |
| Multi-selection download | **PASS, initial:** copying both rows with destination Auto-rename produced two local files containing the exact 24-byte and 47-byte payloads |
| Identical-name folder navigation | **PASS, initial:** both `same-folder` rows opened independently and showed their different marker contents; provider IDs and markers agreed |
| Selected folder rename and trash | **PASS, initial:** rename retained the selected folder ID; normal Delete trashed that folder only, leaving its same-name neighbor and marker intact |
| Selected file permanent delete | **PASS, initial:** the selected renamed 47-byte file became inaccessible by ID; the 24-byte neighbor remained active with exact bytes |
| Selected file rename | **PASS, final:** a freshly seeded 47-byte duplicate retained its ID and contents when renamed to `renamed47.txt`, while the original 24-byte `same.txt` retained its ID and contents; UI completed without a false error |
| Ambiguous directory bulk copy | **PASS, initial:** copying the directory containing duplicate descendant names was refused before destination writes; the destination stayed empty and both source IDs/checksums were unchanged |
| Unique destination overwrite | **PASS, final:** native copy and Overwrite changed a unique 47-byte target to the source's exact 24 bytes; independent listing found exactly one target named `same.txt`, and both original source IDs/names/checksums remained unchanged |
| Shortcut server-side copy | **PASS, final:** the native copy created a distinct shortcut ID with shortcut MIME type and the same target ID |
| Native Google document export and rename | **PASS, final:** native download produced a valid 6,808-byte DOCX containing the fixture text; native rename completed and displayed `document-renamed.docx` |

The initial file rename exposed a real rclone status issue: `moveid` completed
the provider mutation but returned exit 9 under `--error-on-no-transfer=true`.
Its server-side move is counted as a check. The implementation now explicitly
disables that flag for `moveid`; the final native rename and a focused fake-CLI
regression verify the correction. An uncertain mutation was independently
checked rather than replayed automatically.

Automation corrections are separate from product findings. The harness first
used F2 instead of Browsey's Ctrl+R rename shortcut, retried stale AT-SPI nodes,
and added loading/focus waits. Long address-field typing occasionally lost a
prefix during view updates; the harness verified the text and aborted before
Enter. The final overwrite workflow used native folder rows and breadcrumbs.
These harness failures are not counted as successful product operations.

## Regression and performance evidence

- Backend: **856 passed, 0 failed, 19 opt-in tests ignored**, including 13
  focused Drive identity/provider tests and shared transfer/provider suites.
- Frontend: **723 passed across 76 files**; Svelte/type checking reported
  **0 errors and 0 warnings**; frontend lint and naming checks passed.
- Normal application Clippy with `-D warnings`, backend error-hardening guard,
  dependency policy, `git diff --check` and 20 strict documentation consistency
  checks passed.
- Optimized local listing measurement: generated tmpfs directories, five warm
  samples, median **41.602 ms for 10,000 entries** and **396.737 ms for 100,000**;
  maxima 49.698 ms and 401.829 ms. First invocations were 52.115 ms and
  431.183 ms. The measurement covers production listing collection, excluding
  database/IPC/UI and with OS caches not reset. Fixtures were removed afterward.

This is a current-build measurement, not a numerical before/after proof.
Local listing/copy/move implementations, ordinary provider listing layout and
frontend reconciliation algorithms are unchanged. IDs arrive in the existing
Google listing response, without per-entry requests. The HTTP client and ID
allocations are Google-specific. Only Google directory bulk transfers add a
recursive duplicate-path preflight; that deliberate safety cost is documented.

## Supported boundaries and remaining acceptance

Duplicate descendant paths in bulk directory transfers and overwriting
ambiguous source/target names are explicitly refused. Individual duplicate
objects can be selected and transferred to unique destination names. OAuth is
required for direct ID mutations; service-account/ADC and custom directory-name
encoding boundaries are documented in the implementation guide. Concurrent
provider writers remain outside transactional guarantees.

Google quotas/rate limits, interruption/cancellation, large/deep trees, web
restore, external drag and broader lifecycle acceptance remain open. Shared
OneDrive/rclone regression evidence supports common code; it does not certify
these Google-specific cases.

## Cleanup

The owned final candidate window closed normally. Cleanup verified the remote
ownership marker before moving only the generated root to normal Google Drive
trash, then verified its active entry was absent. No global trash purge or
existing-data modification occurred. Private logs and downloaded generated
fixtures remain under `/tmp`; the installed binary retains its original hash.

# Google Drive provider-difference acceptance

Date: 2026-10-07, 20:53–21:11 UTC. The maintainer authorized testing the
production build and prioritized differences from the extensively tested
OneDrive backend. This run reuses shared rclone/transfer evidence and tests
Google-specific identity, naming, native documents and shortcuts.

## Build and isolation

The installed Browsey 1.0.4 executable and environment are the same as the
[earlier production run](2026-10-07-google-drive-production.md). Binary SHA-256:
`244152d699e252bf2425152a879f0080a5f19f240c70fb8c2dfcd27127110622`.
The installation was not rebuilt or replaced; no test IPC was injected.
Native AT-SPI actions and window-bound input targeted only the owned process
and private XDG profile.

A fresh generated, ownership-marked cloud directory contained all test data.
Existing authentication was used in place; no credentials were printed,
copied into the fixture or committed. Provider queries were scoped to the
unique generated root name or verified test-folder IDs. Independent provider
IDs, sizes, checksums and downloaded bytes verified native UI operations.

## Results

| Google-specific case | Result and evidence |
| --- | --- |
| Case-distinct names | **PASS:** native upload retained both `report.txt` and `Report.txt`, with different IDs and exact 31-byte contents; no false conflict |
| Case-only rename | **PASS:** `Report.txt` → `REPORT.txt` preserved its bytes and the lowercase neighbor |
| Exact-name collision | **PASS:** rename to existing `report.txt` was refused with “A file or folder with the same name already exists”; both originals remained unchanged |
| Advanced rename | **PASS:** native batch prefix rename of the two case-distinct files produced `namedreport.txt` and `namedREPORT.txt`, preserving distinct IDs and contents |
| Native Google document | **PASS:** provider metadata confirmed `application/vnd.google-apps.document`; rclone exposed `document.docx` with size `-1`. Browsey listed it with unknown size and downloaded a valid 6,808-byte DOCX containing the fixture text |
| Native document Open With | **PASS:** a private generated handler received the durable 6,808-byte local DOCX; the workspace manifest retained `originalSize: null` and the exported source path |
| Shortcut cloud copy | **PASS:** native copy produced a different shortcut ID with `application/vnd.google-apps.shortcut` and the same target ID |
| Shortcut normal trash | **PASS:** native Delete moved only the copied shortcut to trash; provider queries confirmed its trash state and exact target bytes remained intact |
| Shortcut local download | **PASS:** native download contained the target's exact 44 bytes; the cloud target remained unchanged |
| Two identical names in one folder | **FAIL:** two `same.txt` objects with different IDs and lengths of 24 and 47 bytes appeared as one Browsey row showing 47 bytes. Copying that row downloaded the other object's 24 bytes; repeated after native refresh with the same result |

## Confirmed duplicate-object defect

The fixture contained two genuine Google Drive objects named `same.txt` in
one generated folder, not two case variants. Independent `lsjson --hash`
returned both object IDs and their distinct contents. The second object was
created with a scoped
[Drive multipart upload](https://developers.google.com/workspace/drive/api/guides/manage-uploads)
because ordinary path-based rclone upload replaced the existing fixture file
instead of creating a duplicate. Fixture setup is not counted as a Browsey
workflow.

Reproduction through the installed GUI:

1. Open the generated duplicate folder and wait for its unique ownership marker.
2. Observe one `same.txt` row showing 47 bytes, although the provider has two objects.
3. Select that row, copy it and paste into an empty local test directory.
4. Independently inspect the result: 24 bytes matching the other object,
   not the selected row's 47-byte content.
5. Refresh the cloud folder and repeat into a second local directory; the
   mismatch occurs again.

Both downloads had SHA-256
`e52dfe27744977ad0c196211a67443bfe1d6b84d36ae21dd9eefa5eef1c4dad6`.
Final provider reads confirmed both originals retained their IDs, sizes and
checksums. No rename, move, overwrite or deletion was attempted on the
ambiguous paths.

Source inspection at the time of this installed-build run supported the observed
failure: the then-current rclone
[`LsJsonItem` parser](../../../../src/commands/cloud/providers/rclone/parse.rs)
does not retain the provider ID, and
[`read.rs`](../../../../src/commands/cloud/providers/rclone/read.rs)
constructs child identity from the filename. The frontend's
[`reconcileDirectorySnapshot`](../../../../frontend/src/features/explorer/state/directorySnapshots.ts)
stores incoming entries in a map keyed by `entry.path`, collapsing identical
paths. A subsequent path-based download can therefore resolve a different
object from the metadata shown. This is an identity/integrity defect, not a
conflict-preview success. Duplicate-folder names and destructive operations
were not tested; they must not be inferred safe from this run.

The negative-size document parser already converts negative sizes to unknown
size; it required no change. Fixing only the duplicate row key would be
insufficient: operations must address the selected object consistently, or
ambiguity must be detected and refused before a misleading success.

## Cleanup and scope

The owned production window closed normally. After checking the remote
ownership marker, cleanup moved only the generated cloud directory to normal
trash and verified its active entry was absent. No global trash purge or
existing-data modification occurred. Private evidence, duplicate downloads
and the native-document working copy remain under `/tmp`.

No product code or installed binary was changed during this historical run.
The duplicate-object defect remained open at its conclusion. The subsequent
[2026-10-08 implementation and candidate run](2026-10-08-google-drive-object-identity.md)
fixes and validates object identity, with explicit ambiguous-operation
boundaries; the installed executable remains unchanged.
Shared OneDrive results remain useful regression evidence; they cannot close
Google-specific identity or real-provider quota/rate-limit requirements.
Future Google testing should prioritize provider differences and regression
coverage rather than repeat every shared rclone workflow.

# Native provider behavior

Date: 2026-10-06. Scope: NT5-1 through NT5-5 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md).
Tests use the separately built native-test WebKitGTK candidate, private profile
and generated owned UUID files. No installed app or personal settings are used.
Every completed part is committed separately. Actual device/provider results
apply only to the explicitly tested filesystem, protocol and mounted state.

## NT5-1: USB filesystem and supported permissions

Run `73046996-6f14-4777-b19b-3d0ae8d16bf9` passed every foundation file/folder
operation, within-USB copy/move, both local/USB transfer directions, local undo
and the local/USB Properties tabs. It remains BLOCKED because the extended
access harness shadowed its provider variable during setup; no access mutation
had started. The error was corrected without changing application code.
Fresh run `e13422ce-fd13-409c-9812-135a16694bc3` passes all nine Properties/access
parts plus accessibility. The owned USB path reports **ext4** (statfs magic
`ef53`, exact-path findmnt type). Real read denial and destination write denial
produce actionable failure with zero completed operations and preserved sources
and sentinels. A read-only source copies successfully with mode 0400 preserved;
the captured source modes are restored afterwards. This does not certify Btrfs,
exFAT, NTFS, formatting, unplugging or mount-option-only permission behavior.

Both reports use the same candidate application source/binary. Both have all
four teardown stages and private retention PASS; independent exact PID/start
checks confirm all captured candidate/driver processes gone, and fresh exact-run
retention audits pass. Failed evidence and every recovery file remain retained.
Native policy tests (144), native lint and diff checks pass.

## NT5-2: mounted SFTP/GVFS/FUSE behavior

Run `052e69d7-f7c3-4f45-b5c8-d4d14fc7d394` is native PASS: foundation file/folder
operations, local/network copy and move in both directions, nine progress parts,
five real Cancel parts and four network-specific parts. The selected approved
path identifies SFTP through GVFS/FUSE; no mount/account inventory is used.
One exclusive generated 32 MiB `.bin` file is written in sequential 64 KiB blocks
with a checked 60-second write budget. The default writer's 64 KiB cap stays
unchanged. The large file is never opened for content read, preview or download.

Cancelled permanent-delete confirmation preserves its size and owned-parent
membership. Confirmed delete removes it from the independent FUSE parent and
fresh UI. Private undo metadata remains exactly unchanged, with no content
backup. Complementary source inspection ties this native operation to GIO's
metadata/postorder-delete implementation, which has no file-content read. These
checks do not measure network traffic, large-file throughput or mid-file Cancel.

Renaming a source externally after opening its rename modal reproduces an actual
stale-path error. Browsey rejects the operation explicitly, preserves the renamed
source's bytes and sentinel, creates no requested output and refreshes to exact
independent membership. Transfer task/callback resources release. Provider
service loss, credentials changes and reconnect remain excluded.

All four teardown stages, private retention and fresh exact-run audit pass;
captured candidate/driver PID/start identities are independently confirmed gone.
No fixture/recovery cleanup is performed. Native policy tests (148), native lint
and strict docs checks pass. Candidate application source remains the NT4 final
source; native harness changes are independently hashed in the private report.

## NT5-3: mounted mobile/MTP and thumbnails

Run `515c8887-3561-4134-a1d4-58998c39f009` is native PASS: all foundation operations
in the owned mobile folder and both local transfer directions, six mobile
special-name parts and four thumbnail/metadata parts. A double-quoted filename
is explicitly rejected by the real creation modal on this MTP device; all
existing names/bytes remain unchanged. This is a device restriction, not a
universal MTP rule. No personal camera/device-root enumeration is performed.

Tiny generated 128x96 RGB PNG files produce decoded native private-cache images
in the actual grid. Read-only samples capture cold completion and a third image
added later; actual F5 and list/grid controls refresh metadata. Complete grid
samples maintain Name ascending order, and list order matches independently
known names. Properties reports the late file's independently known size, name
and type. Final raw-byte tree digests preserve all three images and sentinel.
Measured operation timings are retained in the report; no performance threshold
or synthetic late-metadata event is invented. This does not cover a locked
phone, disconnect/reconnect or an unavailable provider mid-file.

All four teardown stages, private retention and fresh exact-run audit pass.
Independent exact PID/start checks confirm all captured processes gone. Native
policy tests (150), native lint and strict docs consistency pass. Fixtures and
private cache/recovery data remain retained; no production build is installed.

## NT5-4: OneDrive operations, working copies and bounded errors

Fresh run `b7406ab8-111a-43db-b1ea-d6a315d6fe5c` is native PASS: all eleven
refresh/upload/cloud-copy/download/conflict/working-copy parts, three exact
cloud-error parts and owned accessibility. The copied private configuration
identifies the approved provider as OneDrive. The complete new run took about
nine minutes, with 180-second readiness and 600-second transfer waits. Actual
F5 refresh exposes generated data; raw content readback checks every destination
and preserved source. Cancel, Skip, Auto-rename and Overwrite all pass.

The first attempt, `d4ea2edc-5957-4c67-863d-79f9927a9179`, completed its first six
parts, then exposed a harness expectation error: unreserved Auto-rename correctly
creates `working-1.txt`, not `working-2.txt`. An identity-checked operator stop
ended the candidate; that report remains FAIL/CANDIDATE_EXITED, not a crash claim
or retroactive PASS. A meaningful harness regression rejects wrong output
acknowledgements. Fresh scoped follow-up
`5a19e994-b41d-4fa9-80b1-b47e00907b6f` then found a real Browsey defect: explicit
Overwrite reported success while keeping old cloud bytes when equally sized
source/destination versions had near-identical timestamps. Its report remains
FAIL/RESULT_MISMATCH; no mutation or fixture was retried in place.

Explicit replacement now forces rclone CLI transfer and per-call RC IgnoreTimes,
covering upload, download, cloud copy and mixed dispatch. This follows the
[rclone ignore-times contract](https://rclone.org/docs/#ignore-times-i), which
bypasses the size/time quick check. New-file immutable/checksum/no-clobber guards,
cross-kind refusal and unrelated directory contents stay protected. Four
content-preservation regressions fail before the fix and pass afterwards; an
additional real-rclone contract check passes on generated local files only.
The final native run repeats the entire fourteen-part manifest on the rebuilt
candidate, independently confirms the overwritten bytes, and verifies that an
equal-size changed original is detected during working-copy upload.

Actual working-copy preparation and Settings upload controls operate in the
private profile. Generated edits save under new cloud names and retain both
original and local working copy, including after the original changes. The
native-only authorization binds one exact cloud source and private manifest/
id/local-path; foreign paths and links are refused before upload. External
editor launch is disabled, so this is no external-editor acceptance claim.

Quota, rate and authentication messages are injected once per exact generated
cut/paste pair, at candidate validation before writes, through the existing
provider classifier. Each UI result reports zero completed/skipped, one failed,
zero not attempted; source bytes and cloud tree remain unchanged and callbacks/
tasks release. Quota and rate classify as `rate_limited`, authentication as
`auth_required`. These checks do not exhaust real quota, change credentials,
exercise live service outages or certify provider retry behavior. Google Drive
and Nextcloud remain outside the approved roots/configuration.

All three reports retain their actual outcomes and recovery data. All four final
teardown stages and fresh exact-run retention audit pass, and independent exact
PID/start checks confirm captured candidate/driver processes gone. Rust passes
819 tests (19 ignored), with the separate real-rclone check passing; frontend
703 and native policy 154 pass. Clippy, formatting, native lint, backend error
guard and blocking Semgrep pass with zero findings. The three blocking rules
also scan the two new native guard/probe files explicitly, with zero findings.

Final native host evidence: Linux x64, kernel `7.2.5-3-omarchy`, Node `v26.10.0`,
GTK `3.24.52`, WebKitGTK `2.52.6`, Norwegian input. Driver SHA-256 values and
per-run tool evidence remain in the private reports. Final harness SHA-256 is
`bae6dd55dfb501eb8151f8c22efcfccfca11c06eb9c9089f203c798798bfdc06`.

## Recorded candidate identities for completed provider parts

All entries are separately staged debug `native-test` builds with dirty source
state recorded before each part's commit. Harness hashes are independently
recorded per private native report. They certify their declared case scope, not
a final committed-baseline or production installation.

| Part | Build baseline | Application input SHA-256 | Binary SHA-256 |
| --- | --- | --- | --- |
| NT5-1 | `4089183cd22c8af6ee35a7c05b78190753d14113` | `d8a896228ce4205c83475d0fcb865ceec77272c7918408a02376e71fac709e0b` | `796c6753803ad0b2563fe49640ae5a179afce75b9c063627a820ce384c7a8695` |
| NT5-2 | `e2d93a106d7dc2d06fb2f88d5a4f6e15732ee68c` | `d8a896228ce4205c83475d0fcb865ceec77272c7918408a02376e71fac709e0b` | `359dfc6701e3c4944b8ccb636cd205f16fcf60913366da9ce6bf009b3799f5d1` |
| NT5-3 | `b0355c3fd45a640aebcc8fbfc2d8346d67260238` | `d8a896228ce4205c83475d0fcb865ceec77272c7918408a02376e71fac709e0b` | `728b37a9af7e578fbd80d3b32c0d948e1c32310f91bc60b4fc16168a986c34a0` |
| NT5-4 | `60482252860fa92165701e26aeefee72d363615e` | `1c1bb8a1818b27cc9fb43e3744fa78533402491c550f122c360407e3f7239073` | `8141b9ef56ad2553223b9e2f6f70f2cb9eb2632fe2d4dbbc95e2297417ce3a2a` |

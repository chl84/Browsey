# Native provider behavior

Date: 2026-10-06. Scope: NT5-1 through NT5-5 in the native suite TODO.
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

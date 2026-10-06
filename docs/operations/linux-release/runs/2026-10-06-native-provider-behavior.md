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

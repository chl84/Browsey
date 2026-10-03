# Local Copy Content Verification

Date: 2026-10-03
Baseline: `120ed04`
Scope: clipboard and undo manual regular-file copies, including local copy/delete
move fallbacks and copy recovery backups/restores. GIO/cloud writers are unchanged.

## Reproduced Defect

Two new `*_rejects_target_edits_masked_by_later_writes` tests failed against the
baseline. A real second file handle rewrote an already-written output prefix at
the 8192-byte write boundary. Browsey then continued writing later bytes and
captured the mixed target's final metadata as if it were its own successful copy.
Both clipboard and undo accepted the edited output. No read/write error or source
change was necessary; version checking after streaming alone cannot detect this.

## Correction and Design Choice

Both local copy engines now hash the written stream with the already-pinned
BLAKE3 dependency. Undo uses a shared `DigestWriter`, hashing only bytes accepted
by successful writes; clipboard hashes each fully written progress chunk.
Neither records a successful receipt until shared content verification succeeds.

`verify_copy_content` checks the pre-sync target version, seeks the still-open
output handle to the beginning and compares the output's digest with the stream.
It rechecks the path/handle version after reading. A fixed 256 KiB buffer and
copied-length-plus-one-byte limit prevent allocating whole files or scanning a
concurrently growing output without a byte bound. Source versions are verified
after readback, before the existing receipt/source-removal gates.

Clipboard cancellation is checked before/between readback chunks. Verification
read errors, seek errors, unexpected length, digest mismatch or detected version
change retain uncertain output and refuse source removal. No destructive cleanup
or automatic retry is introduced. The progress payload keeps original transfer
byte totals; readback does not masquerade as more copied bytes or signal successful
completion before verification. History replay has no new cancellation API.

Private staging/no-replace publication was considered. It would change path
visibility, partial-output handling and publication portability across supported
filesystems, while still needing reviewed post-publication version handling.
This bounded increment instead shares explicit content verification and retains
the current conservative error/recovery model. It is not a final transaction
design or a claim that private staging is unnecessary for future improvements.

## Cost and Capability Boundaries

Verification adds one complete output read pass and hashing of stream/output.
Targets must support an opened read/write handle and seeking; lack of support
fails explicitly rather than allowing unverifiable destructive move completion.
GIO-owned copies and their existing opaque-receipt restriction are unchanged.
Ordinary native same-filesystem rename does not enter this copy path.

Undo's digest wrapper replaces the plain-file `io::copy` fast-path eligibility;
the cost therefore includes userspace copying instead of possible kernel offload.
Do not infer cold-disk, reflink, USB/MTP, network or tiny-file performance from a
warm temporary-filesystem measurement. No performance threshold is enforced in CI.

An optimized disposable 64 MiB, five-sample-per-variant warm-cache measurement
passed. Environment: AMD Ryzen 5 7520U, 8 logical CPUs, Linux
`7.2.5-3-omarchy` x86_64, `/tmp` tmpfs. Reference variants are unverified modeled
native/manual copy plus sync, not measurements of an installed historical build.
Every output is content-checked outside the measured interval.

| Variant | Median | Maximum of five samples |
| --- | --- | --- |
| Unverified native reference | 41.59 ms | 43.70 ms |
| Unverified manual reference | 51.56 ms | 55.74 ms |
| Verified clipboard | 103.08 ms | 109.55 ms |
| Verified undo copy engine | 99.05 ms | 100.49 ms |

Verified clipboard is about 2.00 times the manual reference, and verified undo
about 2.38 times the native reference in this workload. This is a real extra cost,
not a free guard. The run was on a development machine, not an isolated benchmark
host; default tests/build activity may influence it. These results do not establish
a general performance budget. The optimized build used the same copy/verification
code; a later benchmark-profile diagnostic and cancellation-message test assertion
change did not alter the measured copy path.

## Verification Status

The two masked-edit tests now pass for copy and forced move fallback. Readback
fault tests also pass for both engines: read errors, target/source edits, growth,
and clipboard cancellation preserve the appropriate source/output contents.
Shared tests cover empty, single-byte and multi-chunk outputs, wrong expected
digests, successful short writes and propagated writer flush failures.
All six new functional test functions were also rerun directly in the optimized
test binary and passed, independently of the cost measurement.

Final backend maintenance checks passed resource/vendor integrity, dependency
policy, formatting, cargo check, warnings-denied Clippy, typed-error guards and
**618 tests**. Five opt-in tests are ignored in the default suite: four native
acceptance tests and the new cost measurement. The cost measurement was explicitly
run separately and passed; the four native tests were not invoked. Semgrep is
absent and skipped, not passed.

All **296 frontend unit tests in 40 files** passed. Frontend source and IPC
signatures are unchanged; the browser suite was not rerun. All 20 strict
documentation checks, 45 local links and `git diff --check` passed. The narrow new TODO row was
checked after functional/performance verification; the broader active-write/final
check row remains open because removal is not transactional.

## Remaining Boundaries

Another writer can change source/output after final verification, and metadata
timestamps may be coarse or restored. Readback itself is not an atomic snapshot
against concurrent writers. Digests are not stored in undo/removal receipts or
settings and are not logged. Source streams/directory trees are not locked or
guaranteed to represent a consistent point-in-time snapshot.

Installed UI, cold/slow storage, real devices, unplugging, power loss and other
distributions still need separate acceptance. No install, restart, release,
dependency or version change is part of this increment.

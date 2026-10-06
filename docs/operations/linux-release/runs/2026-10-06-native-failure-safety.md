# Native progress, cancellation and failure safety

Date: 2026-10-06. Scope: NT3-1 through NT3-8 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). Each part has
separate acceptance and a separate commit. Real WebKitGTK and Rust run in
exclusive UUID subtrees of the approved local, USB, network, cloud and mobile
fixture folders, using a separate debug candidate with `native-test` enabled.
The installed application and shared devices/services are not controlled.

## NT3-1: visible byte progress

Declared scope: local-to-local and both directions between local disk and
USB/network/OneDrive/mobile: nine cases, each containing a 64 KiB file, a
folder with a 64 KiB file and empty child, and a zero-byte file (27 parts).
Both entire generated sides, exact bytes and unrelated destination sentinels
are independently verified after real clipboard actions. A bounded observer
records displayed activity, byte totals, determinate/indeterminate state and
command completion; it does not initiate operations through direct IPC.

Candidate-only, exact-owned-path checkpoints hold real finalization for 1.8
seconds. Selected local stream writes expose real 16 KiB increments with a
250 ms pause. At most 32 plans are allowed, with five-second holds, 500 ms
write pauses and 64 KiB thresholds. Normal production contains no probes.
Cloud fixture calls allow 120 seconds, UI readiness 180 seconds and transfers
600 seconds; writes are never automatically replayed after uncertain outcomes.
Large-file throughput and universal mid-file callback cadence are excluded.

Unknown cloud metadata previously became a fictional one-byte total. It now
stays unknown. Zero/unknown progress is emitted as indeterminate; successful
mixed-file transfer bytes remain visible while finalization still owns the
operation. Only the command reply completes the frontend activity.

Preparation: 118 native policy tests and native lint pass; the unknown-cloud-
size regression and two bounded-checkpoint regressions pass. These preparatory
checks are not native acceptance.

Local preparation run `eb25fb32-3d15-422f-845a-9240d63f69cf` passed all three
parts and accessibility. Its file displayed 16, 32, 48 and 64 KiB against a
64 KiB total. All four teardown stages passed and all three captured process
identities were independently gone. Fresh retained auditing passed.

Accepted full run `b2ff1d39-4ca9-4f0a-aa13-0bca1b3b6eb7`,
16:08:22–16:14:46 UTC, passed all nine cases/27 parts, accessibility, private
retention and all four teardown stages. No part remains NOT_RUN. Exact source
and destination trees passed independent verification, including both sky
and mobile directions. All three captured processes were independently gone;
a fresh retained audit passed. The recorded audit observed 6,467,809 bytes
and 152 entries before final retention bookkeeping.

Baseline `4b1ed9df02775f561640bf5cf53ea8993fb9b33d`, dirty `true`;
source SHA-256 `83b00a525b69bb9aca76bb4974ac5b1f9a7981bf61c8de8e33943e1e8b566053`;
candidate SHA-256 `c8d1c84bede780605b1bb0b137bb4f3f0ca26ab1b6e02172566e05c54744298b`,
built at `2026-10-06T16:07:37.867Z`; harness SHA-256
`8cfc9219fa784bff4ba33081eb80b8dfa362f56219c528f6445ed5a186339e1b`.
NT3-1 is complete for this declared scope. All 799 Rust tests pass (19 ignored),
Clippy passes with warnings denied, frontend type checks pass, native lint and
118 policy tests pass, the blocking Semgrep scan has zero findings and the
backend error-hardening guard passes. Strict documentation consistency passes.

Retention: the registered-run count limit increases from 64 to 96 because 63
prior runs are retained. The 512 MiB total, 128 MiB per-run reservation and
existing entry/depth/audit limits are unchanged. Prior recovery is preserved.

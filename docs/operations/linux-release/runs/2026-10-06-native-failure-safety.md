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

## NT3-2: copy cancellation

Declared scope: cancellation before the first file write on all nine local-hub
routes; a local mid-file cancellation at an actual 16 KiB write; and cancellation
between three selected files locally and in both local/cloud directions
(13 cases/parts). Mid-file GIO/cloud cancellation and large files are excluded
from this representative native scope. Existing production-core cancellation
regressions cover active rclone children separately; they are not native claims.

All cancel requests use the actual visible Cancel button. Bounded checkpoints
hold at most five seconds and stop immediately when the real token changes.
Every generated source/destination tree is independently compared twice, with
a 500 ms quiet interval. Retained mid-file output must be an incomplete exact
source prefix and must be reported as retained/incomplete. Cancel is not a
universal rollback: completed local batch roots use existing rollback, while
completed cloud roots remain completed and visible in partial counts.

Read-only inspection of Tauri's real event registry checks live transfer
callbacks, including non-enumerable entries. The candidate status reports
actual cancel-registry membership; both must be released on completion.
Mixed transfer cancellation is registered before asynchronous route validation,
so cancellation during metadata validation can reach an existing task rather
than fail with "Task not found" while the transfer continues.

Preparation: 121 native policy tests and native lint pass. Both cancel-registration
regressions and Clippy with warnings denied pass. Blocking Semgrep, the backend
error-hardening guard and strict documentation consistency pass.

Local run `85fa7dad-cc88-4ac1-b26f-268ba1afaef5` passed before/mid-file/between-file
parts and accessibility. Accepted full run
`1ce95333-2279-4cd1-9282-a3831f624ffa`, 16:21:06–16:27:07 UTC, passed all 13
parts and accessibility. Both independent comparisons matched every generated
tree. The local mid-file stop retained exactly 16 KiB of the original 64 KiB
source, reported as uncertain/incomplete. The local between-file case rolled
back its completed first root; each cloud direction retained one completed
root, one failed root and one unattempted root, with matching feedback. All
copy sources and unrelated sentinels survived. Actual cancel-task membership
and live transfer callbacks returned to zero in every case.

All four teardown stages passed, all three captured process identities were
independently gone and a fresh retained audit passed. The report's audit observed
6,208,575 bytes/110 entries before final bookkeeping. No part remains NOT_RUN.
NT3-2 is complete for this declared representative scope.

Baseline `f4eca49f4b2a23ca33dc2e1382e89c317e698780`, dirty `true`;
source SHA-256 `1a1b60a66c4118d43ded0223b94dfac2f4867baad827c39efc329d0fe8bf1b9d`;
candidate SHA-256 `a4c3a1976eea9cd9fd8450bf725ea9247c671635d6b74cd1222ab5c48c6a1fbd`,
built at `2026-10-06T16:20:12.467Z`; harness SHA-256
`75c49d2f9c9555486c92ec2cfd8e2206534d114053f1f8f49b29c8347e167732`.

## NT3-3: interrupted/failed overwrite

Declared scope: local and USB destinations with different original bytes,
cancelling before new writes, cancelling after an actual 16 KiB write and failing
on an owned unreadable source. Cloud coverage cancels before writes in both hub
directions and fails a local-to-cloud overwrite on an unreadable source. Nine
cases/parts use sources up to 64 KiB and a distinct 8 KiB existing target;
permission-denial sources stay within the existing 4 KiB denial-helper limit. Cloud/GIO
mid-file overwrite, directory transactions and provider-internal backups are
excluded from this representative scope.

Real conflict controls choose Overwrite; real Cancel controls stop held work.
Independent full-tree checks require exact source bytes and unrelated sentinels.
Pre-write cancellation/failure must preserve or restore the exact old target.
Mid-file cancellation must retain an incomplete source prefix with truthful
unknown rollback/completion, plus one exact old target in the candidate's private
undo store and an explicit recovery-required diagnostic. The bounded recovery
reader inspects only this run's undo store, no credentials or other profile data.
Owned source permissions are restored through the original captured descriptor
after command completion, without replaying the paste.

Preparation: 123 native policy tests and native lint pass. The recovery verifier
rejects symlinks and distinguishes exact old bytes from unrelated profile data.
Status: NOT_RUN pending native acceptance.

First local/USB run `54f023f6-9ad4-4612-a327-51467a2391f6` remains BLOCKED:
local pre-write and mid-file cancellation passed, including protected exact
old bytes and recovery diagnostics. Denial setup then rejected its 64 KiB
source because the existing descriptor-based denial helper is limited to
4 KiB; no denial or paste was dispatched for that part. Later parts remained
NOT_RUN. The corrected harness uses 4 KiB only for denial cases, preserving
the helper's limit. This is a fresh-run retest, not replay in the failed subtree.

That first run's four teardown stages passed, all three process identities were
independently gone and its fresh retained audit passed. Its report/recovery
remains BLOCKED and unchanged; the audit observed 5,238,326 bytes/75 entries.

Accepted fresh run `229e35eb-0c31-4156-a018-a0def4a85c8a`,
16:32:54–16:36:44 UTC, passed all nine parts and accessibility. Both partial
local/USB outputs retained exact incomplete source prefixes, with one exact
8 KiB original backup each and valid recovery-required diagnostics. Feedback
reported unknown completion/rollback. All seven pre-write/denial cases preserved
exact original targets, including both cloud directions and real cloud overwrite
denial. No part remains NOT_RUN. NT3-3 is complete for this declared scope.

All four teardown stages passed, all three captured identities were independently
gone and a fresh retained audit passed. The report observed 5,480,772 bytes/106
entries. Native lint and 123 policy tests pass; application inputs are unchanged
from NT3-2 and no application fix was required for this overwrite boundary.

Baseline `077b97b2555107786ce0af4a0021504f5ff6e5ac`, dirty `true`;
source SHA-256 `1a1b60a66c4118d43ded0223b94dfac2f4867baad827c39efc329d0fe8bf1b9d`;
candidate SHA-256 `c2acd8866e79c31b870b8481c769355e4f2aaca321ca237fa67cf3bad512b35e`,
built at `2026-10-06T16:30:49.856Z`; accepted harness SHA-256
`cb87ef6ee115424d6726d34a33bf9c3df37c21cb93ec550f73413889545e5005`.

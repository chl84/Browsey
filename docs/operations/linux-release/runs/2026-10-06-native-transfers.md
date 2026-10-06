# Native transfer matrix and conflict verification

Date: 2026-10-06. Scope: NT2-1 through NT2-6 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). Real WebKitGTK and
Rust run in exclusive UUID subtrees of the five approved fixture folders. Only
the separate candidate, private profile and captured drivers are controlled.
Norwegian input and owned fullscreen are used; mobile is part of the matrix.

## NT2-1: within-provider transfers

Declared scope: copy and move on local disk, USB, network, OneDrive and mobile,
with four parts per operation: one file, one empty folder, a nested tree with
an empty leaf folder, and a mixed batch of file/empty folder/non-empty tree.
There are ten cases and 40 parts. Copy uses list and move uses grid;
keyboard and context-menu clipboard/paste actions are both exercised.

Every completed part independently enumerates and reads both entire generated
sides, comparing exact membership, kinds and text bytes. Copy preserves all
source entries; move removes only completed selections. Unrelated source and
destination sentinel files must stay unchanged. The shared verifier bounds trees
to 128 entries/depth four and at most two concurrent reads; only successful cloud
metadata readiness snapshots may repeat. Transport, byte, mutation and session
failures are never retried automatically. All earlier reports/recovery data stay
retained; no real run is deleted.

Status: PASS on all five providers. Harness policy tests pass (99 tests), including
missing output, nested corruption and premature copy-source removal; later cases
stay NOT_RUN after any mismatch. Native lint passes. These checks alone do not
complete an acceptance point.

First run `60aebf90-1ff6-4e32-986d-75f04789af1c`,
2026-10-05 23:50:30–2026-10-06 00:00:06 UTC, remains BLOCKED. All 24 local/USB/network
parts and the cloud file copy passed. Empty-folder cloud copy timed out waiting
for a destination row; later cloud/mobile parts remained NOT_RUN. Post-teardown
read-only metadata confirmed the empty source retained, destination absent and
the copied file/unrelated destination sentinel present. No mutation was retried.
All four owned teardown stages passed, three captured processes were independently
gone and the fresh private audit observed 5,021,113 bytes/95 entries.

That run used baseline `e95560847f90e3b1d5ab45c2805b9d7cef59f545`, clean application
build inputs `c5012abf503ea74b88f9b59e3ce8b710124dab52cc08296058e894e46e62d1fc`,
debug candidate `ba5eda4ccfdf59588dd21cca91489b0de130b5266d40c336e99c7498f1372708`
built at 2026-10-05 23:49:24 UTC and harness
`ec7ae54c8404aeefbcbcfe4224418a3a5aa46f5327319f4f136aee27fbc77f1a`.

Cloud directory copy used `copyto`, which loses empty descendants and does not
materialize an entirely empty source root. It now uses directory `copy` with
`--create-empty-src-dirs`, followed by one planned destination-root `mkdir` only
after a successful copy. Both commands explicitly disable rclone retries. Files
keep their existing route; moves retain server-side directory rename semantics.
The empty-root regression failed before this fix. The regression also checks
nested empty folders, every nested byte and unrelated source/destination bytes
with RC both disabled and forced. A failed copy must preserve its source and
never finalize or retry. CLI-log count assertions account for global arguments.

Part statuses are now persisted on entry and completion before their enclosing
case finishes. A policy regression checks early PASS, failing-part status and
later NOT_RUN parts, so a long remote case is no longer opaque until it ends.
After the fix, all 779 Rust tests pass (19 ignored), Clippy passes for all targets
with warnings denied, and all 99 native policy tests/native lint pass.

Retest `f9b7f47b-d42f-48df-a05e-eb89417d0b08`, 00:04:56–00:18:47 UTC, is PASS:
all ten cases/40 parts, owned accessibility and all four teardown stages passed.
This includes both mobile operations and real OneDrive empty roots/descendants.
Three captured processes were independently gone; a fresh private audit observed
4,978,757 bytes and 100 entries. No case remains NOT_RUN. The first report remains
BLOCKED; this is separate acceptance on the fixed candidate.

The retest uses the same baseline with `dirty=true`, build inputs
`37cfc1c8fc3fc56086d9b57d3534ad49deced1d00446afecc7a6d80fd1e6e608`,
debug candidate `ed7c8dd5d715af6f3c095c0a78fd99c76a576e10d687d1e9f794a3215afbe820`
built at 00:04:47 UTC and harness
`26d24c5b804a48f36eac34997b542ebe00d910403d88cade5727bbb9c7d64704`.
Strict documentation consistency passes all 20 checks.

## Delivery

Each completed NT2 point receives its own commit. No intermediate push or
production rebuild is performed. Test candidates are rebuilt when required by
the source/commit identity guard. After all six points pass, the normal release
build excludes `native-test`; only then are the six commits pushed together.

Large payloads, device/service lifecycle, concurrent writers and cancellation
during writes remain separate NT3/NT4 scopes. No personal data, drive roots,
desktop clipboard, shared service or installed production app is controlled.

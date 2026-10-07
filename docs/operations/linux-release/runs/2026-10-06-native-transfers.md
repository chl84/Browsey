# Native transfer matrix and conflict verification

Date: 2026-10-06. Scope: NT2-1 through NT2-6 in the
[native suite TODO](../../../todo-archive/TODO_NATIVE_TEST_SUITE.md). Real WebKitGTK and
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

## NT2-2: bidirectional local hub

Declared scope: eight ordered routes between local disk and USB/network/OneDrive/
mobile, with copy and move separately: 16 cases/parts. Each operation transfers
one mixed batch containing a file, an empty folder and a nested tree with a file
and empty leaf folder. Both entire generated sides and unrelated sentinels are
independently checked; copy preserves the source and move removes every completed
root, including empty directories. Keyboard/context actions and list/grid remain
real native interactions.

Generated bytes include the route, operation and relative name, so stale clipboard
contents from another route cannot satisfy readback. Policy tests reject that
failure and a move which transfers files but leaves empty source directories.
The initial 101 native policy tests/native lint passed before native execution.
The application build inputs remain unchanged from the accepted NT2-1 candidate.

First hub run `384313aa-3dec-4d4a-bf08-a0957c3c4450`, 00:20:17–00:26:37 UTC,
retains BLOCKED even though all 16 transfer parts, accessibility and four teardown
stages passed. The final local audit rejected four generated network→local files
with GIO copy mode 644. All their parent directories remain mode 700; credentials,
metadata and profile files are mode 600. These copied data permissions do not
make the files reachable by other users.

The audit now records original permissions of regular generated files strictly
below `files/`, while keeping every directory mode 700, current ownership, a
single file link and no special permission bits. Profile, credential, screenshot
and metadata files retain mode 600; `files-sibling/` is outside the exception.
No retained permissions/configuration are changed. New regressions failed before
this correction and verify the metadata/parent boundary, hardlink and special-bit
rejections. The uncertain-run review regression now checks a public data directory
and public report, while preserving an inherited generated-file mode after review.

Explicit review confirmed all captured processes stopped, retained the original
BLOCKED report/recovery data and cleared only its inactive marker. A fresh audit
observed 4,996,829 bytes/162 entries. The first hub run used baseline
`70102310c192fdd030fdca72c72c39b7292a9acb`, unchanged build inputs
`37cfc1c8fc3fc56086d9b57d3534ad49deced1d00446afecc7a6d80fd1e6e608`,
candidate `ed7c8dd5d715af6f3c095c0a78fd99c76a576e10d687d1e9f794a3215afbe820`
built at 00:20:04 UTC and harness
`9d840b4274b12d3580da8e1162d0f9c42b283b45eca8e430b6fbb502e031829b`.
A fresh full hub run was required before checking NT2-2 complete.
All 103 native policy tests and native lint pass after the permission-audit fix;
the application/candidate inputs are unchanged, so no new binary build is needed
for this harness retest.

Accepted hub run `148636ed-5a6c-41aa-9e66-7d88289495ff`, 00:33:14–00:38:46 UTC,
passed all 16 mixed transfer parts, accessibility and all four teardown stages.
No route remains unrun. Independent checks verified all generated bytes, empty
folders, nested trees and unrelated sentinels on both sides, including mobile
in both directions. The three captured process identities are gone; a fresh
retained audit observed 4,997,006 bytes/162 entries. Build identity is the same
as the first hub run above; accepted harness SHA-256 is
`410c5645d013413cc34439c0a548c0844f86e27277fe8eb214b186918ba7ae14`.
NT2-2 is complete; historical BLOCKED evidence remains retained unchanged.

## NT2-3: bounded all-pairs

Declared scope: all 20 ordered cross-provider routes, each with copy and move
separately (40 mixed file/empty-folder/nested-tree parts). The eight accepted hub
routes are rerun on the same candidate as the twelve remaining directions; hub
success alone does not cover network/mobile, USB/cloud or other direct pairs.
Every source/destination byte is route-stamped; entire trees are verified after
each single real native paste, without automatic mutation retries.

Small INFO dispatch receipts record the backend actually selected: GIO,
filesystem directory/rename, owned stream/receipt fallback, rclone CLI or provider
upload/download. They contain operation, entry kind and direct-destination policy,
without paths or credentials. The runner reads only its captured candidate's
private, bounded log, between each operation's markers, and rejects missing,
unknown, rotated or excessive receipts. Receipts establish dispatch; independent
source/destination checks establish outcome. Browsey does not allocate a separate
staging tree for these clipboard transfers; provider-internal temporary files
are not certified. Explicit refusals retain both sides and do not certify writes.
The suite does not turn an unexpected failure into an accepted refusal.

All 105 native policy tests/native lint and 779 Rust tests (19 ignored) pass.
First all-pairs run `fb3cbade-898f-421b-8ad8-61aaa455e25b`, 00:41:51–01:00:08 UTC,
retains FAIL: 30 operation parts passed, then cloud→mobile copy reported an RC
job failure setting the destination partial file's time (`chtimes`, operation
not supported). Nine later operations remain NOT_RUN, including this boundary's
move and reverse route. No failure is reclassified as an accepted refusal.
All four teardown stages passed and all three captured processes are gone;
a fresh retained audit observed 5,210,173 bytes/166 entries.
Read-only independent inspection verified the entire failed copy's cloud source
exactly preserved and its mobile target containing only the original unrelated
sentinel. No app/fixture mutation retry or recovery cleanup was attempted.

Baseline `48e7f8d1ab5fdf16fb34b249ed24e8e702bcccb3`, dirty `true`;
source `e098e66a97363708da4ac1865dd4584126f913f60ea010ce512be21bf417b402`, candidate
`4baac25b3394f3e872db5e1e98b5f43f807d7ffa337d163811a5f3f815eef1eb` built `2026-10-06T00:41:48.255Z`;
harness `2e47d959ff4e96e462eaafd95642eb649dfec53129a42f4a40857c7b39aa33af`.

MTP destination options now disable setting file time for each RC local-filesystem
object and CLI transfer, and disable CLI directory time updates. Other local/
network destinations keep their original timestamp policy. These options are
selected before dispatch; no daemon/global configuration or after-failure retry
is introduced. MTP timestamp preservation is outside this bytes/tree claim.
The [local backend option](https://rclone.org/local/#local-no-set-modtime) and
[RC filesystem-object format](https://rclone.org/rc/#specifying-remotes-to-work-on)
are documented by rclone; RC backend option values are strings.
Three regressions failed before correction: mount classification, file/directory
CLI options and per-destination RC options. The corrected full Rust suite passed
782 tests (19 ignored); Clippy, 105 native policy tests and native lint passed. The full all-pairs retest prioritizes
cloud/mobile in both directions without removing any of the twenty routes.
Accepted all-pairs run `f907e15e-82cc-40a1-bb54-79553a381495`, 2026-10-06T01:04:15.993Z to
2026-10-06T01:27:25.340Z, passed all 40 operation parts, accessibility and four
teardown stages. All 20 ordered routes ran on this one candidate, including
network/mobile and USB/cloud in both directions. No route was refused or left
unrun. All three captured process identities are gone; a fresh retained audit
observed 5,146,682 bytes/186 entries.

Candidate baseline `48e7f8d1ab5fdf16fb34b249ed24e8e702bcccb3`, dirty `true`,
source SHA-256 `cc58e242278f2743aa3193bbb47de69fd552a3469c29856f08b77d7ddfc8db8c`,
binary `95f4bfe691919aa685db7ed22c7548819e2fe29265090d91069efafc07e10b7d` built at
`2026-10-06T01:04:09.303Z`; harness
`f9cb6b7c2c3786a47b1180d0f70320b1700d0fc638c4008dbd2cab57a05236ea`.

Actual routing receipts show GIO or owned-stream files plus filesystem directory
creation for non-cloud copies. Moves selected filesystem rename where available,
and copy/verify/delete for cross-filesystem or unsupported atomic renames. Cloud-bound
files use the provider upload route, cloud-origin files use provider download;
cloud directory transfers use rclone CLI. All record direct destination writes,
with the provider-internal temporary-file limitation above. NT2-3 is complete.

## NT2-4: conflict choices

Declared scope: all four Skip/Overwrite/Auto-rename/Cancel choices on each of the
five providers, plus both local/cloud directions. Copy covers same-kind file and
nested-directory collisions and both file-versus-directory shapes; move covers
the same-kind mixed tree. There are 56 cases/112 independently verified parts.
Reserved `-1` names force Auto-rename to select `-2`; unrelated and destination-only
nested bytes stay unchanged. Skip transfers only nonconflicting roots; a cut
clipboard keeps skipped sources and never clears a newer clipboard selection.
Cancel leaves both exact trees unchanged. Local cross-kind overwrite uses the
existing protected undo backup. Cloud/mixed cross-kind overwrite refuses before
any write because it has no provider recovery receipt; this is an explicit
refusal with exact preservation, not certification of a cross-kind write.

The native baseline confirmed the missing Skip action. The application now wires
Skip through the conflict modal and immutable operation snapshot. Five meaningful
frontend regressions failed before the fix. Both provider and mixed-route Rust
cross-kind tests failed before their guards, covering copy/move and prechecked
inputs without writes. The first Rust attempt had a test-import compilation
error; only the subsequent runnable failures count as regression evidence.
After the guards, Rust passed 784 tests (19 ignored) and Clippy with denied
warnings. Native policy passed 107 tests, including corruption/preservation checks.

The next native run passed the cloud same-kind directory merge, then observed
that the frontend replaced the cross-kind refusal with generic unsupported
feedback. The wrapper now preserves actionable unsupported/invalid-path messages
and typed metadata while retaining friendly connection feedback; it never retries
a rejected write. The native harness retains unexpected feedback verbatim in its
private report instead of replacing it with an opaque rejection label.

Retained BLOCKED run `6d285623-115f-45f0-b588-76df8f7eda48`, `2026-10-06T01:29:50.506Z`–`2026-10-06T01:29:59.795Z`: 0 accepted parts; later cases remain NOT_RUN. All four owned teardown stages passed and three captured process identities are gone. Fresh private audit: 5,074,093 bytes/64 entries. Baseline `db817cf1dbe40aa9d3550bfe823ebe47b52f1108`, dirty `true`, source `cc58e242278f2743aa3193bbb47de69fd552a3469c29856f08b77d7ddfc8db8c`, candidate `95f4bfe691919aa685db7ed22c7548819e2fe29265090d91069efafc07e10b7d` built `2026-10-06T01:29:44.293Z`; harness `3707cd48c675efa527763e8668233509e91d44356da1e5aeb56200d866a494a3`.

Retained BLOCKED run `f88047f2-20c0-462a-b2ef-84631726f1c2`, `2026-10-06T01:33:30.939Z`–`2026-10-06T01:37:39.670Z`: 1 accepted parts; later cases remain NOT_RUN. All four owned teardown stages passed and three captured process identities are gone. Fresh private audit: 5,095,694 bytes/65 entries. Baseline `db817cf1dbe40aa9d3550bfe823ebe47b52f1108`, dirty `true`, source `59b3c55ac7c37eb34e9a353152e966cd85f6e68a8617ba99b72cd481b7506d19`, candidate `19e57c0a47bdcb26c8db0237a0e35f2b521a0a6bebb7f38b9802b3d8bd7b8463` built `2026-10-06T01:33:29.936Z`; harness `3707cd48c675efa527763e8668233509e91d44356da1e5aeb56200d866a494a3`.

Retained FAIL run `4393260d-c275-4310-b90d-4cbc9cd1a580`, `2026-10-06T01:39:58.845Z`–`2026-10-06T01:48:19.161Z`,
passed all three cloud-copy overwrite parts, then failed independent verification
of the cloud move despite GUI completion. Read-only post-teardown inspection
confirmed that new file/nested bytes and destination-only bytes were correct,
but `folder/empty` remained in the source and was absent from the destination.
No write was retried or recovery source reconstructed. All four teardown stages
passed, three captured processes are gone, and the fresh audit after readback
observed 5,078,651 bytes/79 entries. Candidate baseline
`db817cf1dbe40aa9d3550bfe823ebe47b52f1108`, dirty `true`, source `1899598e5ea3f73785e5fb0773406b253c8f0c5841b133324002bf5552a38149`,
binary `a7c4db09f9d20482ae46e81546214d9485bc277217d9b284cad491c1775118d8` built `2026-10-06T01:39:57.856Z`; harness `dc2dfdf58f771ec73c6eb8f33dd2037f0360ae0454c8434de65db24be78d9491`.

Existing cloud-directory overwrite moves now use `move` with empty-directory
creation/deletion flags, one destination-root finalization and an empty-only
source-root `rmdir` with the provider's delete policy. New-target and deliberate
case-only rename routes remain separate. No recursive purge is used. A failed
move never retries or finalizes either side. The merge regression failed before
this correction; successful nested/empty merges and failure-preservation tests
pass with RC forced/disabled. Rust now passes 786 tests (19 ignored), Clippy with
denied warnings, frontend 678 tests, type checking/lint and native policy 107.
Part status and independent verification determine acceptance; GUI completion
labels alone never establish it.

The full conflict run also exposed inherited mode 644 on six generated network
originals copied into protected local undo backup buckets. Their complete parent
chain remained mode 700. Retention now recognizes regular copied backup data
only beneath the exact private undo session/hash-bucket layout; session locks,
recovery markers, credentials and all other profile metadata remain mode 600.
Current ownership, one hard link, no special bits and private directories are
still required. No retained file is chmodded or deleted. Two regressions failed
before this correction and pass after it, including metadata/sibling boundaries,
private parents, hardlink/symlink and special-bit refusal. Earlier BLOCKED reports
keep their original status; explicit review clears only an inactive run marker.
A fresh local/network conflict run and a corrected independent audit of the
complete retained run are required before NT2-4 acceptance. Application/build
inputs are unchanged by this harness-only correction.

Run `5aa57bfd-9b4a-4727-85f7-95129681ed85`, `2026-10-06T01:52:16.101Z`–`2026-10-06T03:09:38.410Z`, completed all 56
cases/112 parts on all seven declared routes. Every functional case and provider
is PASS, including all mobile choices, both local/cloud directions and the
corrected cloud empty-directory move. All four teardown stages passed and the
three captured process identities are gone. Its overall status remains BLOCKED
for the original inherited-backup-mode audit, with no unrun cases. After the
harness correction, an independent audit observed 5,110,731 bytes/743
entries and passed. Explicit review retained its original report/recovery data,
clearing only the inactive marker. No retained file permission was changed.

Baseline `db817cf1dbe40aa9d3550bfe823ebe47b52f1108`, dirty `true`, source
`5cb07ebb5b83d621bfe0437da8c00bd9831058795c440ec5ab865d94a339efb7`, binary `42c56d357ed63f57a38867c3198fe8e1037cc22ef3501fe3c32492747b66a1b5` built `2026-10-06T01:52:15.215Z`;
original harness `dc2dfdf58f771ec73c6eb8f33dd2037f0360ae0454c8434de65db24be78d9491`. The application inputs and binary are unchanged;
the additional native local/network scope specifically retests creation/auditing
of the copied recovery data. All 109 native policy tests and native lint pass.

Native integration retest `4ee0ad41-6661-4591-9c25-12f5f104619e`, `2026-10-06T03:10:24.387Z`–`2026-10-06T03:12:37.895Z`,
passed all 16 local/network cases/32 parts, including inherited recovery-file
modes from new network overwrites. Accessibility and all four owned teardown
stages passed; three captured process identities are gone. The fresh private
audit passed with 4,976,199 bytes/284 entries. It uses the identical
application source/binary above and corrected harness `f7ca03f29e1fd7edc353b08cc6d48cd1fc7d22815f13996004508b99cc4989af`.

NT2-4 acceptance combines the complete 56-case/112-part native functional scope,
its corrected independent privacy audit, and this fresh 32-part integration run.
Both source/destination trees, nested/unrelated bytes, reserved unique names,
source retention/removal and all four choices passed across all five providers
and both local/cloud directions. Six cross-kind cloud-boundary refusals preserve
both sides explicitly. The original overall BLOCKED/FAIL reports and recovery
data remain retained unchanged; no declared functional case is unrun. NT2-4 is
complete. Frontend 678/Rust 786/native-policy 109 tests, type checks, both linters,
Clippy and strict documentation checks pass.

## NT2-5: unsafe targets and bounded aliases

Declared native scope: nine parts on each of the five providers, plus two
OneDrive case-insensitive descendant aliases (five cases/47 parts). Cut in the
same parent, copy/move into the source itself, descendants and an existing
ancestor must reject visibly within a bounded observation. Both entire generated
trees, nested/empty directories and unrelated bytes must remain exact. Copy in
the same parent remains valid only by creating a distinct unique target. Typed
cloud aliases are entered through the real path editor; no unsafe native symlink
fixture is introduced.

The frontend rejects any cut source already in the destination before preview
or dispatch. Cloud writes recheck exact/case-folded source and target relations
even for prechecked inputs, and reject remote roots as entries. Local overwrite
checks ancestors before directory merge. Canonical local parent aliases are
covered separately by a production-core regression on synthetic temporary data.
Cloud name reservation and mixed destination collision handling stop after
50 candidates; unknown write failures are never replayed. Existing legitimate
case-only rename and sibling names remain covered by regressions.

Runnable regressions confirmed two frontend same-parent cut failures, an unsafe
cloud pair with generic failure instead of early invalid-path rejection, and a
local overwrite merge into its source ancestor. The canonical descendant alias
check already rejected safely; the ancestor assertion was the failing part.
After correction, frontend 682/Rust 790/native-policy 111 tests, type checks,
both linters and Clippy with warnings denied passed before native verification.

Initial run `c87847af-ab29-41f5-a4ae-169fca9ca649`, `2026-10-06T03:16:30.854Z`–`2026-10-06T03:27:19.295Z`,
remains BLOCKED after 33 successful parts: all nine local, USB and network parts,
and the first six cloud parts. A scoped rclone mkdir failed while creating the
cloud ancestor fixture, before its GUI operation. Read-only inspection confirmed
the generated base existed and its `Folder` child did not; the original stderr
was unavailable, so no provider cause is inferred. No failed mutation was
retried in that subtree. All four teardown stages passed, three captured process
identities are gone, and its refreshed private audit passed with 5,024,468
bytes/139 entries. Its original report and generated data remain retained.

Second run `754c57d4-6955-4420-94b2-f7f3ac5659b2`, `2026-10-06T03:32:53.026Z`–`2026-10-06T03:47:27.358Z`,
remains BLOCKED after all nine local and all nine ordinary cloud parts passed.
It stopped before submitting the cloud alias path: native Norwegian input sent
period instead of the intended colon in the cloud URI. No alias paste was sent.
The harness now explicitly presses Shift+period for colon and verifies the value
before Enter; global keyboard configuration remains untouched. All four teardown
stages and the fresh private audit (5,001,449 bytes/
153 entries) passed, and three captured process identities
are gone. No app source/binary change was needed for this harness correction.

Fresh independent run `3aad44f2-c9ee-4399-b51f-a558e98f2efd`, `2026-10-06T03:48:10.730Z`–`2026-10-06T03:52:25.327Z`,
passed both cloud alias parts and all nine mobile parts. Together with all nine
ordinary cloud parts in the second run and the unchanged local/USB/network parts
in the first run, all 47 declared functional parts passed on identical app source
and binary; the last run uses the corrected input harness. Accessibility and all four teardown stages passed; three
captured process identities are gone. The fresh private audit passed with
4,947,073 bytes/56 entries. Candidate commit `82373894d6dc29fe28d26a48ab3a04590d619051`
(dirty scoped NT2-5 fixes), source `70cd60190d0db9381582c1f26f4d62cf53be8bfd914f2269d9435b88cf9448be`, binary
`c741085f1e6dae834846c4d619d38895a41626790702b40afb1e4fcb15cd1b62`, built `2026-10-06T03:16:29.794Z`, harness `3259cf29300ad4d9257d15742131c54193d956d4f974ef572d8a4f1ecd431328`.

Frontend 682/Rust 790/native-policy 113 tests, type checks, both linters and
Clippy pass. Before fixes, real regressions exposed same-parent cut dispatch,
cloud unsafe targets and local ancestor overwrite ordering. Canonical descendant
aliases were already safe. NT2-5 is complete; the original setup-blocked run
retains its status.

## NT2-6: partial batches, rollback and refresh

Declared native scope: eight cases on local disk, local→USB, local→OneDrive and
within OneDrive. Network/mobile failure modes are outside this representative
batch scope. Each selected root has distinct generated bytes; a later failing
root and an unattempted root follow the first successful transfer, while Skip
preserves a conflicting root and unrelated sentinels on both sides.

Local and upload cases use a genuinely unreadable newly generated local file.
The fixture holds its owned inode, sets mode 000, independently proves a new
read is denied and restores mode 600 only through that same held inode after
the single paste attempt. No remote permission or service is changed. Local
batch failures retain the existing rollback policy and report zero completed,
one skipped, one failed, one unattempted and one rolled-back root. Uncertain
rollback remains unknown; it never fabricates zero or successful completion.
Cloud/mixed per-root dispatch records exactly the successful roots and reports
one completed/skipped/failed/unattempted; partial cut keeps only remaining
source roots in the clipboard.

Two native OneDrive batch cases use a bounded candidate-only fault before the
exact failing source dispatch, after the first real successful transfer. They
verify partial-batch reconciliation, not an actual provider I/O failure. Two
local cases inject a single exact destination listing failure, armed only by
the declared transfer: one after success and one after a batch failure. The
original transfer error, counts and refresh warning must remain visible together.
Listing error stores are observed before file operations claim refresh success.
An explicit F5 retries only the consumed read fault after feedback is captured;
no second paste/write is sent.

Every injected plan is limited to a generated child, exact operation and command,
authorized before use, and consumed once. Read-only status records only its ID,
armed state and count. The normal production build excludes this fault machinery.
Both complete trees and remaining cut paths are independently checked after
each outcome; cleanup errors cannot hide the primary failure.

Runnable frontend regressions initially failed four cases (66 passed): cleanup
could hide the original cloud/mixed error, local rollback details lacked counts,
and uncertain rollback lacked explicit unknown counts. After the correction,
frontend 689/Rust 796/native-policy 115 tests pass, with 19 existing Rust ignores;
type checks, both linters and Clippy with warnings denied pass. The new debug
native candidate was rebuilt from the scoped source before GUI acceptance.

Initial run `e76b98bc-626a-4b35-98dd-3e439cdfb02a`, `2026-10-06T03:55:34.437Z`–`2026-10-06T04:01:22.486Z`,
remains BLOCKED after all six transfer cases passed. The first refresh case had
already displayed the required success-with-refresh-failure toast, but the
harness incorrectly required an additional persistent error pill after the toast
expired. Its private audit passed with 5,004,480 bytes/
106 entries; all four teardown stages passed and three
captured process identities are gone. No transfer/app fix was required. A real
harness regression failed before removing the extra-pill requirement; it still
requires captured refresh-failure feedback, rejects every unrelated visible error
and verifies the declared fault was consumed once. Its original report and data
remain retained unchanged.

Fresh local run `69cdd95e-2adb-4925-bb02-11ae74041c5f`, `2026-10-06T04:01:58.817Z`–`2026-10-06T04:02:20.071Z`,
passed all three local cases/parts, including refresh failures after success
and after a batch failure. Together with all six completed transfer cases in
the original run, all eight declared cases passed on identical app source and
binary; the local follow-up uses the corrected observation harness. Both entire trees and remaining cut roots
matched after each outcome. The two cloud-source faults in the first run and both refresh faults in
the follow-up were each consumed once, with feedback captured before any read-only F5 reconciliation. Genuine
local read refusals were independently proved and their held inodes restored.
Accessibility and all four owned teardown stages passed; three captured process
identities are gone. A fresh private audit passed with 4,941,450 bytes/
79 entries. Candidate commit `474844e10b2031c05c5301fce8a5bdd139df0870` (dirty scoped NT2-6 fixes),
source `5953275e5d00f42939d45ba7ce58359c45dbd766149da697fe205725bb28ab8f`, binary `7ad29518307f5193119f9d813faaac4a8c1b404a977deb59bb4e0a774f37046b`, built `2026-10-06T03:55:28.395Z`,
harness `3cb9ade02f797a2eb269523ab632be63378f186c4750f323f9b0c932e75e5698`.

Frontend 689/Rust 796/native-policy 116 tests, type checks, both linters, Clippy
and strict documentation checks pass. Before the fix, production frontend regressions
exposed missing partial counts and cleanup suppressing the original error;
rollback certainty, legacy API shape and bounded fault authorization have
separate core regressions. NT2-6 is complete within its declared representative
scope; provider outages and network/mobile failure modes remain separate work.

## Delivery

Each completed NT2 point receives its own commit. No intermediate push or
production rebuild is performed. Test candidates are rebuilt when required by
the source/commit identity guard. After all six points pass, the normal release
build excludes `native-test`; only then are the six commits pushed together.

Large payloads, device/service lifecycle, concurrent writers and cancellation
during writes remain separate NT3/NT4 scopes. No personal data, drive roots,
desktop clipboard, shared service or installed production app is controlled.

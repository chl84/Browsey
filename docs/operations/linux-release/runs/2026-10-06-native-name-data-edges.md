# Native name, content, tree and link edges

Date: 2026-10-06. Scope: NT4-1 through NT4-5 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). Each part is
accepted separately and committed separately. Tests use the separately built
WebKitGTK/native-test candidate and generated owned UUID fixture directories;
there is no control or installation of the normal Browsey application.

## NT4-1: exact special names

Declared scope: all five configured providers, eight special-name files and one
non-empty special-name folder each. Names include spaces, Norwegian letters,
emoji, combining Unicode, both quotes, literal `#`, `%26`, `&`, `_`, and leading
dots/hyphens. Five parts per provider verify list/grid, navigation, copy, move
and renaming an existing emoji source. Three additional local parts test
creation of files/folders and rename with significant whitespace. Seeded emoji
acceptance does not claim native emoji typing; only one Norwegian layout is used.
Exact source/destination membership, name bytes and generated text are read
independently. Remote whitespace restrictions are outside this local Unix scope.
Cloud operation deadlines are 600 seconds and readiness 180 seconds.

Creation/rename previously silently trimmed significant whitespace in frontend
modal/service and backend target construction. Preserve the original non-empty
name through these layers; the existing invalid-component checks still apply.
Meaningful frontend regressions reproduce trimming before the fix.

Retention increases from 512 to 640 MiB total with existing 128 MiB per-run,
96-run, entry/depth/time limits unchanged; all 77 prior runs are preserved.
Initial run `6153f8f1-6a9a-4f45-922d-0d3fed8bb0be` retained five passing
local parts and stopped with a harness argument error before creation. The
harness call was corrected; it was not an application defect.

Run `81044453-32e3-4f59-b63a-afd25e6089a9`, 2026-10-06T18:10:22.913Z–2026-10-06T18:24:31.114Z, passed
local/USB/network/cloud cases (23 parts) before mobile fixture setup refused
the quoted filename with MTP EIO. The overall report remains BLOCKED. No quoted
entry was created, proved by independent owned-parent metadata. This setup
error was not counted as a passing UI rejection.

Fresh scoped local/mobile run `a5252f19-912d-4ca5-96e2-986390eff127`,
2026-10-06T18:26:48.298Z–2026-10-06T18:27:22.814Z, passed all 14 parts and accessibility. Mobile
uses seven supported special names for copy/move/rename and one explicit
creation rejection of the original quoted name. The actual error is
Input/output error; exact parent membership and every existing file's bytes
stay unchanged. Norwegian quote input uses Shift+2 and is checked before Enter.
This is a restriction of this tested MTP device, not universal Android support.

Together the two accepted case scopes cover all five providers and 29 unique
parts: local eight, USB/network/cloud five each, mobile six. The candidate
application build is identical across the two reports; harness hashes differ
because the mobile restriction and real-input case were added after diagnosis.
All four teardown stages and private retention pass on both runs. Every captured
process identity was independently confirmed gone; fresh exact-run audits pass.
The earlier BLOCKED evidence and generated files remain retained.

Candidate baseline `44c16b841d45909cd08c8001f887674ee0330aac`, dirty true; source SHA-256
`b2c9618f4afeeac67aeb3a37307304db8e700895fb3144bb01937b466c2db115`; binary SHA-256
`103887fb54a383f2f1576d019ec78c357aa685d1431a6b515cb0c020327f55bc`. Final scoped harness SHA-256
`1105c7919ff4917b108b995f156660103464cecf37907014c53aad8da3920ed4`; earlier full harness SHA-256 `2a423cff0d56c33f015278f4a61bf38050611ecec30b300501079d479db6a76c`.

Preparation/final checks: 803 Rust tests pass (19 ignored), 699 frontend tests,
131 native policy tests, native/frontend lint, type checking, Clippy with
warnings denied, zero blocking Semgrep findings, error-hardening guard and strict
documentation consistency pass. NT4-1 is complete for the declared scope.


## NT4-2: name limits and representation

Declared scope: reserved `CON`, a 512-character name rejection and a rename
separator rejection on all five providers (15 parts). Reserved names may be
accepted exactly where valid; failures require explicit modal feedback and
unchanged independently read trees. This does not infer identical limits across
providers or certify Windows. Five additional local parts cover a valid 255-byte
UTF-8 name, rejection at 256 bytes, a pathname exceeding the actual Linux 4096-byte
limit, and explicit rejection of invalid UTF-8 filename bytes in listing/search.
The pathname fixture stays below 20 owned levels; files remain tiny.

Invalid-encoding fixtures use one literal `ff` byte below an owned local folder.
After observing the real UI error, the same unchanged inode/bytes are relabelled
under an exact UTF-8 recovery name, including on failure. Original name hex is
recorded; no recovery is deleted. A changed inode/bytes refuses relabelling.

Regressions reproduce lossy listing/search aliases and unvalidated rename leaf
names. Directory listing and recursive search now explicitly reject unsupported
filename encoding with `unsupported_filename_encoding` instead of inventing an
actionable replacement-character path. Single/batch rename validate the complete
leaf before IPC; backend single/batch rename also refuses separators, NUL and dot
components. Cloud child construction refuses NUL.

Runs `4b83d76f-6238-4cc6-a0b5-2fad7fe5a7cb` and
`6182b46c-49ef-4b01-9974-e964937fb714` retain six passing local parts each
and stop BLOCKED on the encoding-error observer. Diagnosis: the global Notice
uses `.notice-error`, and its slide transition can initially make WebDriver's
visible text empty. The observer now waits for readable actual text. A meaningful
red/green policy regression also makes shared readiness refuse a visible global
error notice even when the shell is otherwise idle. This is a harness correction;
neither incomplete run counts as encoding acceptance. Their data, unchanged
relabelled invalid-name files, reports and screenshots remain retained.

Accepted run `3fa19c02-1ac0-43e1-bf79-d780c6a38b24`, 2026-10-06T18:39:35.263Z–2026-10-06T18:41:50.833Z,
passes all five cases/20 parts and accessibility. `CON` is accepted or explicitly
rejected according to the tested provider; no normalization is silently substituted.
The 512-character attempts and slash rename are explicitly rejected everywhere.
Actual local 255/256 UTF-8 byte and >4096-byte path boundaries are verified. Both
actual UI encoding errors are readable; independent readback preserves original
bytes under known recovery names, and subsequent refresh/search succeeds.

All four teardown stages and private retention pass; all three captured process
identities are independently gone and a fresh exact-run audit passes. Baseline
`3e86b3424ea0b4526171a5eac623a09faa0ce7bb`, dirty true; source SHA-256
`1fde73e7c18c78416c7d50ef641b0cb5a2dc56341792188b1711e72bcfdab2c0`; candidate SHA-256 `24f7530f9509165302a26d09da6087d9e2ee3742d1c2b289a0169fb53df07228`;
harness SHA-256 `37c0507961cc657a286a6cf8534f6b6520525399190706367a980c66efd6ab1f`.

806 Rust tests pass (19 ignored), 700 frontend tests, 132 native policy tests,
lint/type checks, Clippy with warnings denied, zero blocking Semgrep findings,
error-hardening guard and strict documentation consistency pass. NT4-2 is complete
for this declared Linux/provider scope.


## NT4-3: independent binary contents

Declared scope: nine local-hub routes (local-local plus both directions between
local disk and USB/network/cloud/mobile), two copy/move parts per route: 18 parts.
Each batch contains zero bytes, one `ff` byte, 4097 generated bytes and 65536
generated bytes. Source and both destinations include distinct unrelated
sentinels. Real list/grid clipboard controls submit each operation once.
Independent metadata membership, raw byte equality and SHA-256 digests verify
all sides, including successful-only move removal and intact previous copies.
Actual task/callback registries must clear. Within-provider binary transfers
other than local are not a separate acceptance claim.

The shared verifier previously decoded every read as UTF-8, including rclone cat.
Add an explicit bounded raw-byte read API while keeping text checks for existing
suites. Binary readback never reconstructs bytes from replacement characters.
Each generated file remains at most 64 KiB; no sparse/large-file transfer or
throughput claim is made. Bounded traversal accepts only the exact expected
shape, never descends into unexpected directories, and awaits both started reads
before surfacing failure.

Run `cf7729c8-5813-4f38-983f-1a9f3ed27b3a` passes 12 parts before failing
cloud-to-local copy on a live mixed-transfer progress listener. Independent
source/destination binary digests already match; this is a Browsey listener
lifecycle defect, not corruption. Successful cloud and mixed paste paths now
release the progress listener while preserving their completion timer/result.
Three frontend regressions fail before the fix and pass afterward. Native
verification also waits for completion activity to disappear. The failed run's
files/report remain retained; teardown, exact-process exit and fresh audit pass.

Accepted run `32fdd97f-e67e-4fdf-aaa1-fc3bc80dc4be`,
2026-10-06T19:03:03.526Z–2026-10-06T19:09:52.118Z, passes all nine routes/18 parts
and accessibility, including both cloud/mobile directions. Exact raw bytes,
metadata sizes, digests, sentinels, successful move removal, previous copies and
released task/callback registries pass. All four teardown stages, private retention,
independent captured-process exit and a fresh exact-run audit pass.

Baseline `16a5b74293851f57426939dd808d370ddaf57744`, dirty true; source SHA-256
`84d7b79b150032f4a0c43f8db1864036792b09b4f688d55694435ce728484bd9`;
candidate SHA-256 `77ece19ee12f50277a4b0800b8ff2c8ed0e36724aa4f0eba650cf616c2371ed8`;
harness SHA-256 `980b1ab237ca025b45590b9b83fe43b5047cf7967206910be9b1fa74912720db`.
703 frontend tests, 135 native policy tests, frontend/native lint, type checks,
zero blocking Semgrep findings, error-hardening guard and strict documentation
consistency pass. Rust sources remain at the verified NT4-2 baseline. NT4-3 is
complete for the declared bounded binary scope.


## NT4-4: bounded tree and listing response

Declared scope: three parts per provider (15) plus three local virtual-list
parts: 18 total. Each provider first copies a six-entry reduced tree containing
empty folders and three tiny binary files, before adding a deeper/wider tree.
The combined source/destination trees have 25 entries, deepest relative folder
at depth eight, and 12 tiny file matches for the actual recursive Search UI.
Exact byte/digest readback checks both trees; completed search/transfer callbacks
and task registries must clear. Returning to normal listing and F5 must respond.

Only local disk receives the 200-entry one-byte-file listing: actual first/last
virtual selection, Ctrl+A and copy of all 200 files, independent byte/digest
checks, then navigation back to the small tree. Verification explicitly stays
under 256 entries/depth eight. This is a bounded representative test, not an
unbounded stress/throughput certification. The native dispatch tree guard also
receives an explicit depth-32 ceiling in addition to its existing 4096-entry
budget. Acceptance evidence follows.


Run `13453540-65f6-4491-bc9b-615b2720079f` passes all local/USB/network parts
and both cloud copy parts (11), then stops BLOCKED on an immediate zero-task
assertion after the full cloud match set is visible. This does not certify
search completion or a product leak: matches stream before terminal completion,
and completed/cancelled workers may still be ending. Their guard is released on
worker exit. The observer now waits at most 180 seconds for actual task and
search/transfer callback release, without replaying operations. Policy tests
require both eventual release and refusal of a task that never clears.

All four teardown stages/private retention pass on the stopped run; every captured
process is gone and a fresh exact-run audit passes. Files/evidence remain retained.
Declared fresh follow-up: local/cloud/mobile plus the local 200-entry case
(12 parts), retaining the already accepted USB/network parts from the same
application source/binary. App build inputs remain unchanged; only readiness
observation changes. No product search defect is claimed from the early sample.


Follow-up `08447a58-deaf-4e7c-abc0-f32657227301` passes all local/cloud/mobile
parts and the local 200-entry case (12), including actual search task/callback
release without replay. The earlier sample was premature cleanup observation;
no product search change is needed. All teardown stages, independent process
exit, private retention and a fresh exact-run audit pass. Final local-only run `75a9b8e4-720a-49cd-95d0-ba42881ca3f3` passes six parts
with the finished harness after an unused import is removed; application
source/binary remains identical. Teardown/private retention, independent exact
process exit and a fresh exact-run audit pass here as well.


These three retained reports establish 18 unique declared NT4-4 parts across all
five providers: USB/network three each in the initial report, local/cloud/mobile
three each plus the three local virtual-list parts in the passing follow-up.
No incomplete cloud search observation counts as acceptance.

Baseline `c75a6ffd2358570f19f4346f6070ed518e36b6ad`, dirty true; source SHA-256
`e03c352a5742211ec7457f066ea2c3a5b7ff01a394af420eb1daa4974b49e5ab`;
candidate SHA-256 `ee36bce7b7182ab041b0d51d641ff34fdf338651d7860a4ee2fd9542ea09825b`.
Initial harness SHA-256 `2b585cfa464fa2ba68b2883d9a549a24198338c3a6f902cdd412cda21f3a2b21`;
passing local/cloud/mobile harness SHA-256 `8237d5904b9b97ad2d61e274004522545c69e46d4ae436ba7b7945935cdcdf34`;
finished local harness SHA-256 `c26d308a528101432b61d210512db5f2f5ef93f7e13e53690109e8e6d98593cc`.
807 Rust tests pass (19 ignored), 137 native policy tests, native lint, formatting,
Clippy with warnings denied, zero blocking Semgrep findings, error-hardening guard
and strict documentation consistency pass. Frontend app sources retain the 703-test
verified NT4-3 baseline. NT4-4 is complete for the declared bounded scope.


## NT4-5: exact owned local leaf links

Declared representative scope: local Unix only, five parts: list/grid relative
and broken link rendering, explicit clipboard copy rejection of each symlink,
hard-link copy to an independent inode, and hard-link alias move retaining the
original inode. Other providers' link capabilities remain DEFERRED; no directory
or outside referent is authorized. These tests verify Browsey's existing
unsupported-symlink clipboard feedback, not a new symlink transfer feature.

The native-only guard is extended before functional fixtures: at most four
symlink/group declarations, exact owned local paths, single-component sibling
relative referents, no parent links, and exactly two known hard-link aliases
(including one predeclared move slot). Owner/referent state, link spelling and
all alias device/inode/counts are rechecked; unknown outside aliases fail from
the link count without discovering/reading their paths. Traversal remains
4096-entry/depth-32 bounded. Production builds exclude this guard.

Fixtures contain a tiny owned text referent, a missing sibling referent, and six
binary bytes under two known hard-link names. Independent no-follow metadata,
whole membership and byte/digest checks preserve both link targets and unrelated
source names after every part. Private captured link identities support exact-run
post-teardown auditing. Default regular readers/metadata/credentials remain
single-link/no-symlink; a narrow captured-group reader handles the two approved
hard aliases. Presence of link-policy metadata refuses automated deletion,
regardless of run age/status. No retained link/referent/recovery is deleted.
Accepted run `867ce419-ebe2-44e0-bba6-99f6163b11bc`,
2026-10-06T19:52:02.040Z–2026-10-06T19:52:21.551Z, passes all five declared parts and accessibility.
Actual list columns identify both symlinks as Link; both clipboard attempts give
Browsey's explicit unsupported-symlink feedback and preserve whole membership,
referent/alias bytes and empty destinations. Hard-link copy creates a distinct
single-link inode; moving the approved alias preserves the original inode and
exactly two known names. All four teardown stages/private retention pass; every
captured process is independently gone and a fresh exact-run link-aware audit passes.

Baseline `e3335ff9fb9d07da692200457ee5a5aa3f2e4589`, dirty true; source SHA-256
`d8a896228ce4205c83475d0fcb865ceec77272c7918408a02376e71fac709e0b`;
candidate SHA-256 `1a32eb0e7d234ae861fd32da37b815192795bf6fb4440af73ae8955f2e4f563e`;
harness SHA-256 `0e2bab84857b4f2561394efd777a06fe2918f39ba7ae0309da0e18b4006c598e`.
811 Rust tests pass (19 ignored), 143 native policy tests, native lint/formatting,
Clippy with warnings denied, zero blocking Semgrep findings including the newly
tracked guard module, error-hardening guard and strict documentation consistency
pass. Frontend app sources retain the verified 703-test baseline. NT4-5 is complete
for the declared local Unix scope. The five NT4 sections establish 90 unique
native parts; duplicated follow-up checks and stopped observations are not added
to that count. Normal production build/push are recorded separately in the local
ignored production artifact receipt.

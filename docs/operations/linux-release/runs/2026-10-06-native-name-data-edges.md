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

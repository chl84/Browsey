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

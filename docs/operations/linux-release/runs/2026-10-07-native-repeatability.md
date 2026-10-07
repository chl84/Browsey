# Native repeatability and acceptance

Date: 2026-10-07 (Europe/Oslo). Scope: NT7-1–NT7-5 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). One commit per part;
a clean production build and push follow completion. No normal app installation.

## NT7-1: explicit tiers and shared smoke

Run `2381b2f2-1a8c-4d50-9961-36e0f66c7858` is real native PASS: three
existing foundation cases (exact Unicode input after modifiers, file/tree copy,
copy undo/redo), plus AT-SPI accessibility. Total run time is 11.6 seconds,
including setup and teardown; this is descriptive, not a performance gate.
GTK 3.24.52 / WebKit 2.52.6, Arch Linux, private Norwegian X11 layout.
Candidate baseline `0bed504`, source SHA
`9179ac91b95bc56ec79e93ede5d472750d6c32d488c59821f0f5a68870928373`,
binary SHA `31cd3341d7b81a74b9560ceb34d5e2b7c5c82a6168c846803a68979cb8846dc8`;
harness changes were uncommitted for this acceptance, as recorded privately.
All four owned-process teardown steps, private desktop teardown, outside
namespace exit and strict retention PASS. Generated source/destination bytes
are independently checked. Private reports retain the exact harness/tool SHAs.

Initial run `8699c540-88b7-4471-b267-2ab4bdf9147d` is BLOCKED: WebDriver
dropped æøå on the private US map. The next startup
`6a6fa033-e2ed-4d2b-b40b-b0bdac18266c` is BLOCKED because the candidate
environment intentionally omits the layout annotation. Read the annotation
from the wrapper instead, set only its authenticated private display and verify
the actual XKB map. Neither attempt is acceptance or a reproduced Browsey
defect; both reports/fixtures and confirmed teardown remain retained. No host
layout, theme, font or desktop-service setting changes.

Three new policy tests and the existing native policy suite pass (160 tests);
native lint passes. Policy/mock selection is explicitly separate from actual
native acceptance. Provider, edge, stress and lifecycle selection reuse existing
case bodies and require explicit scope; ordinary smoke enables no service
exception or network. The finite count cap is 160 (previously 128), preserving
126 earlier registrations and the bounded new runs. Total/per-run byte and
traversal/audit limits stay unchanged; nothing is automatically deleted.

## NT7-2: lean fresh/reused profile repetition

Run `bdaa761a-98a8-46e0-92fb-21804f095228` is real native PASS: seven
declared cases plus accessibility, 25.0 seconds total. The three smoke bodies
run in fresh generated directories, then once again after one deliberately
owned app/driver restart in the same private profile. Compact density survives
restart; earlier source and undo-target trees remain exact. New app PID/start
identity and a fresh fixed port pair follow all four confirmed old teardown
steps. Final four-step teardown, private desktop closure, namespace exit,
resource release and strict retention PASS (86 entries / 804,953 bytes).
Together with NT7-1 this covers two accepted fresh profiles and one reused profile.
No provider repetition, implicit skips, mutation/session retry or timeout increase.

Earlier run `01e0f117-f09d-4234-b88f-270b2415d266` is BLOCKED: the test
incorrectly expected the temporary grid toggle to persist across startup. The
native scope explicitly seeds default list view on every launch. Reuse the real
Settings density control instead; no Browsey product defect was reproduced.
The failed report and generated files stay retained. Existing restart/report/
retention policy tests pass (23 tests), including finite count/byte reservation,
rejection of unresolved uncertainty, old-run reuse and unconfirmed teardown.
Native lint passes. Registry audit: 131 registrations, 554,731,715 accounted
bytes, no UNCERTAIN entries; all finite limits remain enforced. Candidate built
on `0954e32a` with the same app-source SHA as NT7-1; exact identity is private.

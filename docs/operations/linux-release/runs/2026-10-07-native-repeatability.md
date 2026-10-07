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

## NT7-3: measured, bounded local baseline

Run `33396cdd-fd03-4782-aff5-8c0f31d8e99e` is native PASS: 15 declared
parts plus accessibility, 57.1 seconds total. Shared appearance, four feedback
and three cancellation bodies preserve exact generated files, fallback behavior,
zoom geometry and stopped writes. Main four-step teardown, private desktop
closure, namespace exit and retention PASS (792 entries / 12,813,617 bytes).
Fresh private app/thumbnail cache; warm generated file/OS page cache. GTK/WebKit
and host match NT7-1; actual input map is verified private US for these gestures.

| Metric / measurement boundary | Samples | Observed ms |
| --- | ---: | --- |
| First 50-entry list, guarded navigation + WebDriver | 1 | 232 |
| First decoded thumbnail, observer before grid toggle | 1 | 128 |
| Warm 50-entry grid navigation + all decoded thumbnails + WebDriver | 3 | 297 / 385 / 391 |
| Real Ctrl-wheel dispatch → observed CSS size (guards/Python/driver included) | 6 | 336–465; median 366 |
| Delivered pointer → first matching rAF layout, list/cancel | 39 | median 1; p95 10; max 20 |
| Same, list/copy | 51 | median 1; p95 7; max 16 |
| Same, grid/cancel | 44 | median 1; p95 2; max 8 |
| Same, grid/copy | 57 | median 0; p95 5; max 10 |

These are descriptive observations, not backend-only listing/render timings or
final compositor latency. Browser clock resolution can produce zero-ms samples;
that does not establish zero real input latency. Workloads: 48 tiny RGB PNGs + two
fallbacks, one late PNG, 120 filler files for each drag scenario. No content
download/provider stress or system-wide cold-cache reset.

The first cancel wall-clock metric mistakenly included the existing verifier's
five-second toast-expiry wait. Functional cancellation passed; that raw metric
is **not cancellation latency** and is superseded by trusted-click/DOM timestamps.
Run `263805e0-ce37-447e-8745-61c8f61abcf1` is a fresh native cancellation
PASS (three parts + accessibility, 22.7 seconds total). Before/middle/between-file
acknowledgement is 5 / 3 / 4 ms; terminal cleared activity with failure toast is
30 / 42 / 25 ms (one sample per checkpoint). Source/destination readbacks repeat
after quiescence; backend tasks and callbacks release. All teardown/retention PASS.
Full verification round trips stay separately recorded at 5.1–5.2 seconds.
The exact generated five-second checkpoints are interruptible on cancellation;
no injected hold or toast dwell is labelled normal disk throughput/response.

Three timing-policy tests reject absent/negative/unbounded evidence, stale frame
pairs and toast-expiry timing confusion; eight focused timing/progress/cancel/
thumbnail tests and native lint pass. Budgets remain **NOT_AGREED**, with no hard
regression threshold. Measurement delivery is complete; budget gating requires
a separately agreed repeated host baseline. No product change or new reproduced
Browsey defect. Both raw runs remain unchanged and retained.

## NT7-4: isolated opt-in CI definition

[CI contract](../../../testing-native-ci.md) and the manual native-smoke workflow
require a new single-job Linux image/runner, unprivileged namespace isolation,
sealed exact GTK/WebKit/driver metadata and executable hashes, and one exclusive
local-only generated root/profile/registry. A personal/shared workstation is not
a runner. No provider credentials, PR/fork trigger, mock fallback, normal app
installation or write/publishing token. Only allowlisted redacted receipts upload;
raw reports/config/logs/screenshots and recovery data remain private.

Read-only preflight matches GTK 3.24.52 / WebKit 2.52.6 and all four expected tool
hashes on this accepted host. Actual CI preparation on this ordinary host is
correctly refused before writes, preserving the existing local config hash.
Three CI policy tests pass: scope refusal, tool/engine/version mismatch and
redaction/incomplete-native-evidence rejection. Native lint and workflow YAML
parse PASS. Image labels/environment markers are provisioning contracts, not
independent proof that infrastructure is ephemeral.

Local equivalent run `b064913a-272e-4237-a76f-f184f83e3179` is real native
PASS on a rebuilt candidate at `ea195413`: three shared smoke cases plus AT-SPI,
11.4 seconds total; all four process teardown steps, private desktop closure,
namespace exit and retention PASS. The CI redaction helper accepts its complete
actual report without private paths/credentials. **Dedicated CI execution is
NOT_RUN**; this deliverable defines and locally verifies the shared smoke/contract,
not runner deployment or provider/device/distribution acceptance. No new
reproduced Browsey defect.

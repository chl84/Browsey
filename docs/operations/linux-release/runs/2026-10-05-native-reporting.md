# Native case and provider reporting verification

Date: 2026-10-05. Scope: NT0-5 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). App source and
foundation operation semantics did not change. Fresh owned local/USB fixtures,
synthetic policy fixtures and exact approved-path preflight checks were used;
personal windows, installed Browsey and shared services were not controlled.

## Candidate

Baseline commit: `65963b6da318b644dc02d18744068e6c8a3989aa`, with uncommitted
harness changes (`dirty=true`). The separate debug `native-test` candidate was
rebuilt at 18:48:10 UTC. Its executable/build inputs match the
[foundation follow-up](2026-10-05-native-foundation-fixes.md). Host: Linux x64,
kernel 7.2.5-3-omarchy, Node 26.10.0, GTK 3.24.52, WebKitGTK 2.52.6,
tauri-driver 2.1.0. Norwegian input was used without changing the desktop layout.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `ace1f947bb64f6060cee12d9a916c7a6564d37e6ad0741c4ef0c9b7606b018f4` |
| Build inputs | `990431fe11805252511c2f9d89c0ea29ef924dc578ab3653dec9b40f0a9c2f49` |
| Reporting/fault/normal-run harness | `30c87c622b2a29f152c605ec19fccbaeb406cb88a9e1147a03039b3fc465d7d1` |

## Delivered behavior

Report schema 3 declares stable case IDs, ordered provider dependencies and
case-specific capability requirements before setup. Source reads/removal and
destination writes/creation are distinct. Summaries include provider setup,
case counts, partial results and capability required/passed case IDs. They do
not assume support, turn untested scopes into N/A, or certify broad capabilities
beyond the declared bounded cases.

Setup stages record identity/dependencies, ports, approved roots, exclusive runs,
private profile, artifacts, host/tool evidence, cloud initialization, driver,
session and candidate identity. Approved-root metadata/credential checks still
happen before writes. Exclusive local owner/report creation precedes later provider/profile
creation, so partial setup retains its report. Earlier authorized-plan preflight
failures print structured BLOCKED reports with NOT_RUN cases and no artifact path.

Transfers retain separate file/directory parts and UI-attempt/acknowledgement
state. Independent verification establishes each PASS. A later blocked part does
not erase an earlier PASS; later cases/parts remain NOT_RUN. Case/setup/part
recorders reject repeated attempts, and the runner stops at its first failure.

App-reported errors and candidate process loss remain explicit FAIL evidence.
Independent assertion mismatches are candidate-result FAIL evidence, requiring
diagnosis before attributing a root cause. Fixture/setup/driver failures are
harness BLOCKED; unclassified UI failures are undetermined BLOCKED. A timeout
alone cannot claim a reproduced app defect. Teardown still preserves every
failure and blocks overall/provider PASS when closure/exit is uncertain.

## Verification

Commands used with the existing approved ignored local configuration:

```bash
npm --prefix frontend run test:native:policy
npm --prefix frontend run lint:native
BROWSEY_NATIVE_INPUT_LAYOUT=no node frontend/e2e-native/report-acceptance.mjs
BROWSEY_NATIVE_INPUT_LAYOUT=no node frontend/e2e-native/lifecycle-acceptance.mjs
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --targets local --a11y
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --targets local,usb --a11y
```

All 40 policy/orchestration tests passed, including ten new reporting regressions.
They cover requirements/direction on all five provider kinds, DEFERRED versus
NOT_CONFIGURED, setup failure before UI, a retained real synthetic local report
after injected later-provider setup failure, partial transfers and no retry,
explicit unrun directory parts, distinct app/result/fixture/unknown failures,
preserved unrelated completed provider cases, undeclared/mismatched case denial
and uncertain teardown. The initial synthetic setup fixture exceeded the socket
path limit; shortening only its generated temporary root resolved that test.
Native lint and strict documentation checks passed.

The reporting wrapper verifies these exact expected failures:

| Scope | Observed result | Reporting assertion |
| --- | --- | --- |
| Missing-driver preflight | Structured BLOCKED, all seven local cases NOT_RUN, no owned run/process | PASS |
| Injected private-profile setup failure | Owned report BLOCKED before app start; all seven cases NOT_RUN | PASS |
| Injected directory verification read failure | Four prior cases PASS; copied file part PASS; directory part BLOCKED after UI acknowledgement; two later cases NOT_RUN | PASS |

The partial-copy run independently retained matching generated source and
destination bytes for both file and tree after teardown. All captured owned
process exits were confirmed. The injected error is fixture I/O, not an observed
Browsey defect; its native report remains BLOCKED despite passing fault assertions.

All five NT0-4 native lifecycle fault assertions also passed with the new report
format and exact original failure kinds. Driver exit, read-only case timeout and
closure rejection/hang remain BLOCKED; deliberate candidate fatal termination
remains FAIL. The read-only timeout was previously classified FAIL in schema 2;
schema 3 correctly classifies this injected harness wait as BLOCKED. No prior
report or historical run result was changed.

The normal local run from 18:49:29 to 18:49:55 UTC passed eight cases, all setup
stages and all four owned teardown stages. The local/USB run from 18:51:19 to
18:52:21 UTC passed 17 cases and all 16 file/directory transfer parts. Local and
USB foundations, all four ordered local/USB copy/move routes, Norwegian input,
local undo/redo and AT-SPI passed. Local has 12 relevant passing cases and USB
has nine; four route cases belong to both. All their declared capabilities have
passing case evidence. Exact captured process exits were independently checked.

Mobile was separately selected with local and accessibility. Its exact previously
approved GVFS path was absent. Both the metadata check and run stopped before
creating fixtures or starting a candidate; the structured run report retained
BLOCKED, the failing approved-root-mobile setup stage and all 17 cases NOT_RUN.
That is successful unavailable-provider reporting, not mobile operation acceptance.
No device-root discovery, bridge restart or connection action was attempted.

When that same approved folder became available, the later
[mobile retest](2026-10-05-native-mobile-retest.md) passed all 17 local/mobile
cases on clean committed code. The earlier preflight above retains BLOCKED;
the successful fresh run is separate evidence.

## Limits and retention

NT0-5 is checked for the delivered reporting and focused native verification.
NT0-6 retention remains open. The normal local/USB report explicitly defers
network, cloud and mobile; the mobile preflight is a separate BLOCKED report.
Five-provider requirement mapping is policy evidence, not a fresh five-provider
native pass. Earlier mobile/OneDrive foundation acceptance remains historical
evidence in its original records. This increment adds no device lifecycle,
watcher, conflict, cancellation, large-file or other-distribution acceptance,
and no daily-driver parent row closes.

Private reports, profiles, credentials, screenshots, logs and generated recovery
fixtures remain in ignored owned runs. Preflight/fault aggregates remain ignored
under `target/native-test`; no raw reports, device identifiers, machine paths or
binaries are committed in this redacted record.

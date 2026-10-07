# Native startup and teardown fault verification

Date: 2026-10-05. Scope: NT0-4 in the
[native suite TODO](../../../todo-archive/TODO_NATIVE_TEST_SUITE.md). Only fresh generated
local runs and test-owned child processes/loopback listeners were used. The
installed Browsey, personal windows, shared services and external providers were
not controlled. This completes harness fault coverage, not device lifecycle or
release acceptance.

## Candidate and changes

Baseline commit: `9b921d5acfafbaadd81b6d66ee23a3ee36a10b5b`, with uncommitted
harness changes (`dirty=true`). The separate debug candidate was rebuilt with
`native-test` at 18:32:27 UTC; app build inputs and executable match the
[foundation follow-up](2026-10-05-native-foundation-fixes.md). App source did not
change in this increment. Linux x64, kernel 7.2.5-3-omarchy, Node 26.10.0,
GTK 3.24.52, WebKitGTK 2.52.6 and the same tauri-driver 2.1.0 were used.
The normal local retest used Norwegian input; no layout change was needed.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `ace1f947bb64f6060cee12d9a916c7a6564d37e6ad0741c4ef0c9b7606b018f4` |
| Build inputs | `990431fe11805252511c2f9d89c0ea29ef924dc578ab3653dec9b40f0a9c2f49` |
| Final fault and normal-run harness | `ebcabb15182f2b7da3a1c65158427e1018385094f5f64afe2650dcf1d8997cd4` |

Startup reserves both ports before fixtures and rejects occupied ports without
adopting or stopping their listeners. Readiness/session creation requires socket
ownership by the spawned tauri-driver and its exact WebKit child. WebKit and
Browsey ownership captures executable, private profile, run marker and process
start time. Rechecks reject changed identity and avoid signaling a reused PID.
Case boundaries detect process exit; failure screenshots are bounded and owned.

Teardown calls session deletion once, with a five-second limit, then attempts
each owned process shutdown even if closure or another shutdown failed. TERM
escalates to KILL after bounded waits; an actual exit must be confirmed. The
report preserves each step. Uncertain closure/exit blocks a would-be PASS and
never overwrites an existing FAIL. These checks are for a trusted test session,
not atomic protection against a malicious same-user process swap.

## Verification

Commands used, with the already approved ignored local configuration:

```bash
npm --prefix frontend run test:native:policy
npm --prefix frontend run lint:native
BROWSEY_NATIVE_INPUT_LAYOUT=no node frontend/e2e-native/lifecycle-acceptance.mjs
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --targets local --a11y
```

All 30 policy/orchestration tests passed (11 new lifecycle regressions), and
native lint passed. The focused regressions verify:

- Occupied second port releases the first reservation; a foreign listener stays
  alive and receives no readiness request, including a preflight race.
- Real owned loopback listener readiness, failed driver spawn, normal/signaled
  early exit, driver death during a hanging readiness request and startup timeout.
- TERM-resistant owned processes escalate to KILL while an unrelated sentinel
  stays alive. Executable/profile/run mismatches cannot grant signal ownership;
  changed identity and simulated PID reuse cause no signal.
- Failed/hanging session closure is attempted once; tauri-driver, WebKit and
  candidate shutdown still run. All teardown failures remain visible, and an
  unconfirmed exit never becomes PASS. A timed-out read-only action runs once.

The opt-in native wrapper then passed five fault assertions on separate fresh
local sessions. No UI file operations ran in those sessions:

| Injection | Required observed failure | Native report | Fault assertion |
| --- | --- | --- | --- |
| Driver terminated before readiness | `DRIVER_EXITED` | BLOCKED | PASS |
| Scoped candidate killed after handshake | `CANDIDATE_EXITED` | FAIL | PASS |
| Single read-only case wait timed out | `CASE_TIMEOUT` | FAIL | PASS |
| Session deletion rejected | `SESSION_CLOSE_FAILED` | BLOCKED | PASS |
| Session deletion hung | `SESSION_CLOSE_TIMEOUT` | BLOCKED | PASS |

Each report retained its expected failure and exit code 1. Every captured owned
process exit step passed, and the wrapper independently confirmed those exact
PIDs no longer had an executable. The closure faults inject a rejection/hang in
the runner's deletion callback; they do not claim to reproduce a WebKit defect.
Fatal candidate loss is deliberately simulated with SIGKILL, not an observed app
crash. Passing these assertions does not turn their native reports into PASS.

A fresh normal local run from 18:37:32 to 18:37:58 UTC passed all eight cases:
Norwegian exact path/name input, create, rename, permanent delete/cancel,
within-local file/tree copy and move, local copy undo/redo and AT-SPI accessibility.
Independent generated-file checks passed. Session closure, tauri-driver exit,
WebKit-driver exit and candidate exit each passed, and exact captured PIDs were
independently confirmed gone. The report is PASS; USB, network, cloud and mobile
remain explicitly DEFERRED in this local report.

## Limits and retention

NT0-4 is checked for the delivered harness hardening and focused verification.
NT0-5 capability/partial-batch reporting and NT0-6 retention remain open. Broader
provider acceptance is preserved in earlier records; no additional mobile or
other provider file-operation coverage is claimed here. Device disconnect,
mount/reconnect, shared-service interruption, watcher behavior, large files and
other distributions remain outside this scope. No daily-driver parent row closes.

Private reports, driver logs, screenshots, profiles and generated recovery data
are retained only in ignored owned runs. The wrapper's aggregate is ignored
under `target/native-test`. No credentials, raw reports, fixture paths or binaries
are included in this redacted record.

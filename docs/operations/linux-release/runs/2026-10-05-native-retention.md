# Native privacy and retention verification

Date: 2026-10-05. Scope: NT0-6 in the
[native suite TODO](../../../todo-archive/TODO_NATIVE_TEST_SUITE.md). This increment
changes the harness and documentation; Browsey application source is unchanged.
Native verification uses fresh owned local fixtures. Cleanup deletion is verified
only on generated temporary fixtures; existing real runs and remote data remain.

## Candidate and evidence

Baseline commit: `a565ef6a74710b17e84f746f2a31f3f123b83a66`, with uncommitted
harness changes (`dirty=true`). The separate debug `native-test` candidate was
rebuilt at 19:12:46 UTC. Host: Linux x64, kernel 7.2.5-3-omarchy, Node 26.10.0,
GTK 3.24.52, WebKitGTK 2.52.6 and tauri-driver 2.1.0. Norwegian input was verified
without changing the desktop layout; one layout suffices.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `ace1f947bb64f6060cee12d9a916c7a6564d37e6ad0741c4ef0c9b7606b018f4` |
| Build inputs | `990431fe11805252511c2f9d89c0ea29ef924dc578ab3653dec9b40f0a9c2f49` |
| Final harness and local run | `2e895b88a56de59b093d0858d62b30d63100ef53bae88caa58054891f119fbeb` |
| Earlier reporting/lifecycle fault harness | `b51184568b710fbd81dcd2d80cce51ba9d1795b41bc0359957a3e0de566188d8` |

The final harness additionally rejects cleanup when runtime sockets remain;
that guard is verified with a live synthetic Unix socket. Fault evidence above
predates this cleanup-only guard. Source/executable identity checks still reject
stale builds; these dirty-baseline runs do not claim committed-release acceptance.

## Delivered behavior and findings

Machine configuration and test-only credentials require current-user ownership,
mode `600`, regular files and one hard link before parsing. Scoped processes use
umask `077`; directories require `700`, persisted files `600`. Private JSON writes
reject links and unsafe metadata before truncating an existing owned file.
The machine configuration was initially `644`; an exact-file permission-only
change set it to `600`. The approved test-only cloud credential was already `600`.
Neither file's contents were changed or published.

The first native run completed all eight UI cases but retained overall BLOCKED
because AT-SPI creates a mode-777 Unix socket despite umask `077`. Metadata
inspection found it behind private `700` directories. The audit now accepts
current-user sockets behind an entirely private directory tree while keeping
regular-file permissions strict. Cleanup refuses runs containing sockets because
ownership of any remaining sidecar process is not established.

Explicit review of that one registered uncertain run confirmed teardown, exited
recorded processes and bounded private metadata, then restored byte accounting
and cleared its matching inactive marker. All recovery files and its original
BLOCKED report were retained; no historical result became PASS.

Private ignored registry metadata reserves 128 MiB before a new run, accounts
actual audited local bytes afterward, and limits newly registered runs to 20 and
accounted bytes/reservations to 512 MiB. Each local tree audit checks a 128 MiB,
10,000-entry, depth-32 and 10-second elapsed budget. Uncertain accounting stops
new runs. These guards are not OS quotas, hard I/O deadlines, remote accounting
or an inventory of older unregistered fixtures.

The [suite guide](../../../testing-native.md) documents the explicit audit,
review, plan and cleanup commands. Cleanup requires one matching registered
UUID/nonce/root, a native PASS report with passing cases, confirmed teardown,
exited recorded processes, no active marker/runtime socket, and at least seven
days of retention. A reviewed plan hash is mandatory. Changed plans stop before
deletion; entry changes or I/O failure during deletion stop without retry and
record uncertainty. Postorder removal rechecks identities, keeps control metadata
until other entries are gone, and preserves the approved root and unrelated data.
Failed/blocked, young, active, uncertain, legacy and multi-provider recovery data
remain. There is no automatic expiry, bulk/force mode or remote cleanup.

## Verification

```bash
npm --prefix frontend run test:native:policy
npm --prefix frontend run lint:native
BROWSEY_NATIVE_INPUT_LAYOUT=no node frontend/e2e-native/report-acceptance.mjs
BROWSEY_NATIVE_INPUT_LAYOUT=no node frontend/e2e-native/lifecycle-acceptance.mjs
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --targets local --a11y
```

All 58 policy/orchestration tests passed, including 18 privacy/retention additions.
These cover credential copying into a private profile and denial before run
creation, owner/mode/link checks, non-truncation of hard-linked files, inherited
umask, real confined sockets, metadata budgets, Git exclusions, exact synthetic
cleanup preserving unrelated sentinels, failure/age/active/process exclusions,
legacy/mismatched ownership, changed plans, injected entry replacement, uncertain
review and count/byte/lock guards. Native lint, strict documentation checks and
relative links passed.

The reporting wrapper passed missing-driver preflight, injected setup failure and
partial-transfer assertions. Both owned fault reports remained BLOCKED with a
passing private-tree audit; the partial-transfer screenshot was `600` and its
generated source/destination recovery bytes were retained. All five lifecycle
fault assertions passed with their exact expected original failure kinds:
driver exit, timeout and closure failures remain BLOCKED; deliberate candidate
termination remains FAIL. Every captured process exit was confirmed. Closure
failures keep their active markers and are ineligible for cleanup.

The final normal local run, 19:18:47 to 19:19:15 UTC, passed all eight cases:
input, create, rename, deletion/cancellation, file/tree copy/move, undo/redo and
AT-SPI. Setup, private-tree audit and all four teardown stages passed. Independent
checks confirmed captured process exits and the absent active marker. The final
CLI audit counted 79 entries and 4,979,127 bytes; run/profile/artifact directories
were `700`. Cleanup planning correctly rejected this young run without deletion.
The registry accounted 11 new retained runs and 44,694,570 bytes with no unresolved
uncertainty. These totals include retained failures and separate fresh retests.

## Limits

NT0-6 is checked for the delivered harness work and focused verification. No real
retained run was deleted, no remote deletion was attempted, and older runs were
neither enumerated nor adopted. Other providers remain deferred in the fresh
local run; prior mobile/provider acceptance stays in its original records.
This adds no navigation, large-file, device lifecycle or release acceptance,
and closes no daily-driver parent row. Private reports, profiles, credentials,
screenshots and generated fixtures remain outside committed evidence.

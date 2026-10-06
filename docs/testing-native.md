# Scoped native acceptance suite (Linux)

[Development](development.md) · [Daily-driver validation](operations/linux-release/daily-driver-validation-checklist.md)

This suite drives a separately built Browsey window in real WebKitGTK, with the
real Rust backend. Existing Playwright tests remain fast, mock-IPC frontend tests;
they do not replace native acceptance. AT-SPI supplements WebDriver for the owned
window's accessibility tree, reusing `tests/support/native_fixture_a11y.py`.
Pointer/keyboard/accessibility interaction and independent file-content checks
are complementary; neither alone proves all file-operation behavior.

The [2026-10-05 foundation run](operations/linux-release/runs/2026-10-05-native-foundation.md)
records the first scoped native results and input/guard stabilization. Functional
TODO items remain open until their declared provider scope passes.

The [foundation fixes and retests](operations/linux-release/runs/2026-10-05-native-foundation-fixes.md)
record complete case coverage on all five providers and both local-hub directions,
plus the fresh local history retest. The full report remains FAIL for its history
wait; the local retest is PASS. These separate scopes do not certify a release.

The [startup/teardown fault verification](operations/linux-release/runs/2026-10-05-native-lifecycle.md)
records NT0-4 policy regressions, five injected native failures and a fresh normal
local foundation run. Fault reports retain FAIL/BLOCKED; passing fault assertions
do not count as provider file-operation acceptance.

The [case/provider reporting verification](operations/linux-release/runs/2026-10-05-native-reporting.md)
records NT0-5 setup/partial-transfer regressions, scoped operation retests and
explicitly blocked mobile preflight. Requirements describe the tested cases;
they do not assume broad provider support.

The [mobile retest](operations/linux-release/runs/2026-10-05-native-mobile-retest.md)
passed all 17 local/mobile foundation cases once the same approved folder became
available. Its schema 3 provider/part results and owned teardown are PASS; the
earlier unavailable-folder preflight remains a separate BLOCKED result.

The [privacy and retention verification](operations/linux-release/runs/2026-10-05-native-retention.md)
records NT0-6 permission/Git regressions, explicit cleanup of synthetic owned
runs and fresh native local/fault checks. Existing real recovery data is retained.

The [owned-folder navigation verification](operations/linux-release/runs/2026-10-05-native-navigation.md)
records NT1-1 passing list/grid cases on all five providers, including menu
Refresh, F5, owned history/breadcrumbs and independent generated-file readback.
It also records the shared OneDrive refresh correction and retained earlier
blocked reports. Fullscreen acceptance covers the owned candidate only.

The [listing and recursive search verification](operations/linux-release/runs/2026-10-05-native-listing.md)
records NT1-2 passing all 75 parts across the five providers, including recursive
OneDrive search, combined filters/reset, hidden files and list/grid transitions.
It also records the missing-root and stale-search-draft fixes and retained
earlier blocked attempts.

The [selection verification](operations/linux-release/runs/2026-10-05-native-selection.md)
records NT1-3 list/grid selection and exact selected-file copies on all five
providers, plus a representative 200-file local virtualized list. It also
documents the bounded retention-count increase without deleting recovery data.

The [creation verification](operations/linux-release/runs/2026-10-05-native-creation.md)
records NT1-4 creation, input/collision rejection, preservation and focus checks,
including the shared name validation and interactive cloud-folder collision fix.

The [editing, history and Properties record](operations/linux-release/runs/2026-10-05-native-editing-history-properties.md)
defines the NT1-5 through NT1-8 cases and records their native outcomes, application
fixes and retained earlier attempts.

The [transfer matrix and conflict record](operations/linux-release/runs/2026-10-06-native-transfers.md)
tracks NT2-1 through NT2-6, with separate acceptance and commits for each point.

The [native test suite TODO](todo/TODO_NATIVE_TEST_SUITE.md) prioritizes harness
stability, basic operations, transfer boundaries, failure safety and edge cases.
It tracks suite work; accepted candidate outcomes stay in the daily-driver
validation checklist and its run records.

## Safety and ownership

Only explicitly approved, **already existing** directories named exactly
`ai_agent_testfolder` are eligible. There is no home/mount/account discovery and
no creation of missing approved roots. Earlier approvals with other spellings
do not apply. Network/mobile targets need a working absolute GVFS/FUSE path; a
GIO URI that Nautilus can open is not proof that this local path exists. Resolve
only the exact approved folder, e.g. `gio info --attributes=standard::type URI`;
do not list device roots to search for fixtures.

Every run creates an exclusive `.bnt-<UUID-without-hyphens>/files` subtree inside each
approved root. All app mutations and verification reads are limited to these
owned subtrees, not existing test-folder contents. Private HOME/XDG directories,
undo storage, credentials, logs and screenshots live in the local run. Short
`r/` and `t/` runtime/temp directories avoid native Unix socket path-length limits.
Fixtures
are small generated text/binary files, not personal data. No installed app is launched,
replaced, restarted or controlled.

The opt-in Cargo feature `native-test` adds a fail-closed IPC command/path guard.
Unknown commands, implicit paths and traversal are rejected. Local symlinks are
rejected by default; only the explicit local `links` suite may declare exact
owned sibling leaf referents. Parent/directory links and outside referents stay
forbidden. Unknown hard-link aliases are rejected.
Automatic mount/mobile discovery, system clipboard import/export, global trash,
formatting, connection/mount actions and external application launches are not
allowed. The directory watcher is disabled because its normal discovery/fallback
can leave the approved scope; tests explicitly refresh. Undo/redo recheck owned
local trees. These restrictions are absent from normal builds.

This is a guard for a trusted test session, **not an OS sandbox** against a
malicious same-user process replacing paths during I/O. Do not concurrently edit
the owned run. Desktop libraries, system packages and the existing D-Bus services
are still used. Do not enable this feature in installation/release workflows.

## Setup

Requires Linux GTK3/WebKitGTK 4.1, Node >=22.19, locked frontend dependencies,
`tauri-driver`, and **a WebKitWebDriver compatible with the app's WebKitGTK**.
The runner manages an external driver directly to keep session/profile ownership
explicit; see [Tauri's manual setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/).
It does not install system packages or download browsers automatically.

Copy `frontend/e2e-native/config.example.json` to the ignored
`frontend/e2e-native/config.local.json`. Enter only maintainer-approved absolute
test-folder paths, or `rclone://Remote/ai_agent_testfolder` for cloud. Leave missing
providers `null`; they remain NOT_CONFIGURED, not PASS. The local root is mandatory.
Set this machine configuration to mode `600` before using the runner. Private
metadata/credential files must be regular files owned by the current user with
one hard link; symlinks, other owners and broader permissions are rejected before
parsing. The runner does not silently chmod an existing configuration.

For cloud, the maintainer must supply a **test-only** `rclone.conf` containing only
the approved remote, inside the approved local folder, mode `600`. Set its path
as `rcloneConfig`. The suite copies it into its private profile; it never opens
the user's normal rclone config, imports other accounts or inherits credential
environment variables/SSH agents. Token refresh can update the private copy.
Treat both config files as credentials; never commit them or raw reports.

If the maintainer explicitly authorizes reading an exact existing personal rclone
config, use `provision-rclone.mjs --approved-existing-config SOURCE LOCAL_ROOT REMOTE`
once to extract only the approved provider into `LOCAL_ROOT/rclone.conf` (mode 600).
It never overwrites the test config, modifies the source or prints credentials.
This narrow exception does not authorize reading any other personal file. The
normal runner still accepts credentials only from the approved local test folder.

```bash
npm --prefix frontend ci
npm --prefix frontend run test:native:policy
bash scripts/dev/test-native-linux.sh --plan
bash scripts/dev/test-native-linux.sh --build
bash scripts/dev/test-native-linux.sh --check
bash scripts/dev/test-native-linux.sh --run --a11y
```

`--plan` performs no target probes/writes. `--check` probes exact local paths and
tool availability without creating fixtures; it does not test cloud connectivity.
`--run` is the explicit permission to create/mutate generated fixtures inside the
approved roots. Optional `--targets local,usb` selects a bounded subset; excluded
configured targets are DEFERRED. Configured but unavailable targets stop a run;
there is no silent skip or fallback to the installed app.
`--suite foundation` is the default; `--suite navigation` selects the separate
NT1-1 navigation cases. `--suite listing` selects NT1-2 sorting, column filters,
hidden generated files and scoped recursive search in both views on all selected
providers, including cloud. Fault injections remain restricted to the foundation.
`--suite selection` selects NT1-3 selection and exact-copy cases in both views
on all five providers, plus a 200-file virtualized local list case. Large-list
virtualization is represented by local disk; it is not separately certified on
the remote providers. Ctrl toggles, Shift ranges, arrows, Escape, empty-space
clicks, navigation restoration and filtered select-all use real input. Copies
are sent once and independently checked for exact membership and bytes; these
checks are not a backend invocation receipt. NT1-6 covers repeated requests.
`--suite creation` selects NT1-4 on every selected provider. Each case records
24 parts covering empty file/folder contents, Enter/Create, Cancel and Escape,
empty/whitespace names, separators and dot components, same-kind and cross-kind
collisions, focus restoration and correcting a rejected draft in grid. Every
rejection independently verifies exact entry membership and existing/nested
sentinel bytes. Expected modal rejection text is checked explicitly; it is not
treated as successful idle or suppressed as an unrelated application error.
Escape follows the shared input behavior: first blur a focused text field, then
close the modal with the next Escape. Creation-name checks run before IPC for
both local and cloud paths, so rejection does not rely on the native-only guard.

`--suite editing` combines NT1-5 through NT1-8: each selected provider runs eight
mixed/no-selection file-operation parts, twelve rename parts and three Properties
parts. Local disk additionally runs 108 history parts, including move, rename,
supported permanent deletion, overwrite, redo invalidation, the 50-action limit
and one owned candidate restart. `--suite fileops`, `--suite rename`,
`--suite properties` and `--suite history` select those groups for diagnosis.
History is local only and requires the selected local target. Its restart reuses
the private profile and generated fixtures, persists every captured identity,
and launches the next session only after all old owned teardown stages pass.
An unconfirmed closure blocks the run without an automatic restart/retry.

Properties opens through Ctrl+P or the context menu and checks file, non-empty
folder and mixed selection, all four tabs and restored focus. Recursive item
counts include the selected directory itself. Unmeasured cloud folder totals
stay unknown; selected-file bytes alone are not a complete mixed-selection total.
Ownership/permission capability checks do not change controls or approve system
principal discovery. Exact generated-tree membership/kinds/bytes are checked
after every editing attempt, including cancellations and expected rejections.
The permanent-delete warning distinguishes supported local Undo in the current
session from irreversible cloud/network deletion; it does not promise remote Undo.

`--suite transfers-within` selects NT2-1: copy and move on every selected provider,
each with file, empty-folder, nested-tree and mixed-batch parts. Ten cases/40 parts
cover all five targets. Both generated sides are independently checked after
every part, including unrelated sentinels, empty directories and source
preservation/removal. These cases are separate from the smaller foundation cases.

`--suite transfers-hub` selects NT2-2: eight ordered local↔USB/network/cloud/mobile
routes with copy and move separately (16 cases/parts). Every operation transfers
a mixed file/empty-folder/nested-tree batch, including an empty nested leaf.
Route/operation/name-specific generated bytes detect stale clipboard origins;
exact readback checks both sides after each operation.

The local retention audit keeps every directory mode 700 and all profile
metadata, credential and artifact files mode 600. Regular generated transfer
files strictly below `files/` may retain their copied permissions (for example
GIO mode 644); their private parent chain prevents access by other users. Their
owner must still match, with one hard link and no special permission bits. No
retained permission is silently changed, and the exception excludes sibling
paths or metadata outside the generated data root. The same inherited-file-mode
allowance applies only to copied recovery data under exact private
`profile/data/browsey/undo-sessions/session-PID-STAMP-ATTEMPT/HASH-SEQUENCE/`
buckets. Every containing directory stays mode 700; session locks, recovery
markers and all other profile metadata retain mode 600.

`--suite conflicts` selects NT2-4: all four Skip/Overwrite/Auto-rename/Cancel
choices on all selected providers plus both local/cloud directions. Copy covers
same-kind and both file/directory shapes; move covers mixed file/nested-directory
collisions. Every outcome independently verifies both entire generated trees.
Skip preserves conflicting roots and retains skipped cut paths; reserved names
force unique naming past the first suffix. Cancel preserves both sides. Local
cross-kind overwrite keeps protected undo backups, while cloud/mixed cross-kind
overwrite explicitly refuses before any write. Existing cloud directories merge
without losing destination-only bytes or empty source descendants. Failed jobs
never replay; source-root cleanup is empty-only after a successful move.

`--suite guards` selects NT2-5: nine same-target/self/descendant/ancestor and
valid unique-copy parts per selected provider, plus two typed case-insensitive
OneDrive descendant aliases (47 parts on all five providers). Unsafe transfers
must show an explicit error within a bounded observation, and leave the entire
generated tree unchanged. Same-parent copies create distinct targets. Synthetic
production-core regressions cover canonical local parent aliases separately;
that guard suite does not introduce symlinks. The separate opt-in local link
suite verifies exact generated leaf links without relaxing its default refusal. Cloud name reservation and mixed
collision handling stop after 50 candidates; unknown writes never replay.
`--suite guards-aliases-mobile` selects only both cloud aliases and the nine
mobile parts for a scoped follow-up; it does not imply the omitted parts passed.
Norwegian native cloud URI input explicitly uses Shift+period for colon and
verifies the complete input before submission.

`--suite batches` selects eight representative NT2-6 cases on local disk,
local→USB, local→cloud and within cloud. Newly generated local files can use a
held-inode read denial, independently proved and restored after one paste.
Cloud source and local refresh cases use bounded candidate-only exact-path,
one-use faults, authorized before use. They test reconciliation rather than
provider outages. Both entire trees, counts, skipped/unattempted roots and
remaining cut paths are checked; refresh failures preserve the original error.
Normal production builds exclude the injection machinery.

Tree verification runs at most two independent reads at once and waits for both
started children before surfacing a failure. Cloud metadata readiness permits at
most four successful tree enumerations, separated by one-second gaps, before
reading any bytes; each fixture CLI call has a 120-second limit, with a
60-second I/O timeout and a 15-second connection timeout. Transport
errors, corrupted bytes and mutation dispatch are never automatically retried.
Unexpected directories are recorded but not traversed. Duplicate entries always
fail verification, including MTP aliases; they are never silently deduplicated.

The listing cases compare file order against independently read fixture sizes,
modified minutes, names and extensions. They combine all four column filters,
reset individual filters and all filters in grid, toggle hidden files and check
case/extension queries, empty results, unsubmitted search drafts and view changes.
A matching sibling fixture outside the search start directory must never appear.
Final independent readback verifies every generated file remained intact.
Cloud search walks live provider metadata below the start directory, using the
same query language and cancellation token as filesystem search; it does not
download file contents or use cached folder listings as completed search results.

Driver paths can be explicitly set with `BROWSEY_TAURI_DRIVER` and
`BROWSEY_WEBKIT_DRIVER`; otherwise only system tool directories and
`target/native-tools/bin` are checked. A local driver can be installed with
`cargo install tauri-driver --locked --root target/native-tools`. The native
WebKit driver must be supplied separately. Ports 4444/4445 must be free.
If a Cargo shim is unconfigured, `BROWSEY_CARGO=/absolute/path/to/cargo` selects a
working toolchain for `--build` without modifying global tool settings.

## Startup, teardown and fault verification

Before creating fixtures, the runner checks both ports together and releases its
reservations even when the second port is occupied. After spawning its own
tauri-driver, it proves that port 4444 belongs to that process and port 4445 to
its direct WebKit-driver child before probing readiness or creating a session.
A listener racing preflight is not adopted. Driver startup is bounded at 15
seconds, with readiness requests bounded at one second and no operation retries.

The read-only handshake identifies Browsey before UI mutation. Captured WebKit
and Browsey ownership requires the exact executable, private XDG data directory,
run marker and Linux process start time. These are checked again before signals;
an exited or reused PID is not signaled, and changed identity fails closed.
Case boundaries check all three processes. Failure screenshots have a five-second
limit and require the still-owned candidate window.

Teardown attempts session deletion once, with a five-second limit, then separately
confirms owned tauri-driver, WebKit-driver and Browsey exit. Tauri-driver gets
TERM then KILL with three seconds for each; WebKit/Browsey get 1.5 seconds for
each signal. Every teardown step runs even after an earlier failure, and its
result is retained. Failed/uncertain closure or exit changes a would-be PASS to
BLOCKED; an existing FAIL remains FAIL. No cleanup targets a foreign listener,
installed Browsey or personal window.

Run the explicit NT0-4 fault regression after building the scoped candidate:

```bash
node frontend/e2e-native/lifecycle-acceptance.mjs
```

It creates five separate owned local runs, with no UI file-operation mutations:
driver termination, candidate fatal termination, a read-only case timeout,
failed session closure and hanging session closure. It checks the exact expected
failure and confirmed owned process exits. The native reports remain FAIL/BLOCKED
and exit 1; the wrapper's PASS means the failure was handled as expected. It
retains private reports and an ignored `target/native-test/lifecycle-results.json`.
Individual injection uses `--run --targets local --lifecycle-fault NAME`; help
lists supported names. Other providers and `--a11y` are rejected in fault mode.
Occupied-port, spawn-error, TERM-resistant process and unconfirmed-exit regressions
use generated child processes/loopback listeners in `test:native:policy`.

## Case and provider reports

Report schema 3 declares every case before setup, including optional accessibility
with a stable ID. Each case has required capabilities per provider. Transfer
requirements distinguish source reads/removal from destination writes/creation;
local undo and input remain local requirements. Provider summaries include setup
steps, case counts, case/part results and capability `requiredBy`/`passedBy` IDs.
Capability status covers those declared cases only. An unfinished required case
cannot certify its capability; no provider is silently declared unsupported or
given N/A without separate evidence.

Setup records dependency/build identity, port checks, each approved root and owned
run, private profile, artifacts, tool evidence, cloud initialization, driver,
session and candidate identity. Approved-root metadata/credential checks still
precede writes.
Once exclusive local ownership is recorded, the report is created before other
run/profile setup, preserving later setup failures. Failures before that point
print a structured `Native preflight report` with BLOCKED and declared NOT_RUN
cases, without claiming an artifact path. Invalid config/arguments that cannot
produce an authorized plan still stop before a run is declared.

File/tree transfer cases retain separate `file` and `directory` parts. Each begins
NOT_RUN/NOT_SENT, records its UI attempt and acknowledgement, then independent
verification. A completed file stays PASS if directory verification fails; later
cases/parts stay NOT_RUN. UI STARTED/ACKNOWLEDGED describes the test's interaction
and completion wait, not a backend write receipt. A failed or partial case/setup
step cannot be run again by the recorder. The runner stops at the first failure.

Failure evidence is explicit: `app-reported` is an observed Browsey error;
`candidate-result` is an independent assertion mismatch; `candidate-process` is
loss of the owned app. Setup/fixture/driver failures are `harness`; unclassified UI
failures are `undetermined` and BLOCKED. A timeout alone does not establish an app
defect. Result mismatches still require diagnosis before attributing a root cause.
Completed unrelated cases remain visible, while unfinished provider scopes and
uncertain teardown cannot be reported as accepted PASS.

Run the opt-in local reporting regression with the staged candidate:

```bash
node frontend/e2e-native/report-acceptance.mjs
```

It verifies missing-driver preflight, injected private-profile setup failure and
a fixture-read failure after the file/tree copy UI completes. The native reports
remain BLOCKED, with completed work and recovery fixtures retained. The wrapper
checks exact outcomes and owned process exit, and independently reads only those
generated copy fixtures. Its ignored aggregate is `target/native-test/report-results.json`.
Individual setup/partial-transfer injection uses `--run --targets local
--report-fault NAME`; other providers, accessibility and lifecycle fault mode are
rejected. These injections test reporting, not an observed Browsey defect.

## First-phase cases

The same cases run against local disk, USB, network, cloud and mobile:

- Create a file/folder through the context menu and check the actual result.
- Rename a file/folder, preserving contents and removing the old path.
- Cancel permanent deletion, verify preservation, then confirm file/folder deletion.
- Copy and move files and a nested folder within each provider; verify destination
  bytes independently, with source preservation/removal as appropriate.
- Copy and move the same fixtures in both directions between local disk and each
  other configured provider. `matrix: "all-pairs"` extends this to all ordered pairs.
- Local copy undo/redo, checking both source and destination.
- With `--a11y`, check that AT-SPI exposes the exact candidate's file list; missing
  accessibility is a failure, not a skipped successful case.

UI mutations use real controls/shortcuts, not direct file-operation IPC or
synthetic DOM events. One read-only test-only IPC handshake verifies the correct
candidate/session before any UI mutation. File setup/verification uses guarded
local filesystem operations or an explicit private-config rclone command.
Paste verification waits up to 360 seconds for the one original request, covering
the backend's 300-second transfer deadline and final listing reconciliation;
ordinary readiness waits use 60 seconds. App errors fail immediately, and a
timeout stops the run without resending the mutation.
The shared input case checks `/`, `_` and `æøå` with exact value/focus checks before
Enter, then independent file verification. W3C actions explicitly press/release
Shift for `_`. Only one keyboard layout is required for the first increment;
non-BMP emoji entry remains outside this input acceptance. Set
`BROWSEY_NATIVE_INPUT_LAYOUT=no` (or the actually verified layout) to annotate a
run; this variable does not change the desktop layout. Temporary desktop layout
changes require maintainer authorization and restoration afterward.

## Owned-folder navigation cases

```bash
bash scripts/dev/test-native-linux.sh --run --suite navigation --a11y
# With explicit maintainer approval for the owned candidate's window geometry:
bash scripts/dev/test-native-linux.sh --run --suite navigation --fullscreen --a11y
```

The navigation suite runs the same two list/grid cases on each selected provider,
including mobile. Each uses four small generated folders and four text files.
Eight recorded steps cover the owned bookmark, view selection, nested folders,
back/forward, an in-scope breadcrumb, empty-folder/menu Refresh behavior, a new external
fixture becoming visible after F5, repeated visits and switching views without
changing folders. F5 retains its folder-refresh action in both production and
native-test builds; it must not reload the WebView as a browser page. Each navigation checks the exact current path, active view
and displayed entry paths/empty state. Independent readback verifies generated
file bytes and directory preservation. Saved bookmarks must equal this session's
owned roots; outside helper paths and observed paths fail closed. Only in-scope
breadcrumbs are clicked. Ancestor breadcrumbs and personal Places are excluded.

Long native paths can clip breadcrumbs in a tiled window. `--fullscreen` is an
explicit opt-in for Hyprland: the existing helper validates the captured candidate
PID, executable and private profile, requires one matching window address, sets
that window fullscreen once, and checks its state. It uses the documented
[Hyprland window dispatcher](https://wiki.hypr.land/Configuring/Basics/Dispatchers/).
The runner checks owned process identities before and after. No desktop config,
window rule or global shortcut is changed. Normal teardown closes the owned
candidate. A full-window pass does not establish narrow-window breadcrumb layout
acceptance; NT6 layout and keyboard-focus coverage remain separate.

Not yet covered: native drag/drop and cross-instance behavior, large-file progress
and cancellation, overwrite/recovery, archives/passwords, trash, formatting,
disconnect/reconnect, normal watcher behavior and other distributions. Small-file
success does **not** prove visible progress or cancellation. Expand these in shared
cases only after the foundation runs reliably.

## Evidence, Git and retention

The shared build is staged under ignored `target/native-test`, not a worktree or
build cache per run. Its manifest records commit, dirty state, build time, profile
and executable/build-input SHA-256. The build captures inputs before and after
compilation, rejecting edits during the build. Preflight and runs reject a
different commit, changed executable, changed build inputs or old manifests.
Documentation and harness edits do not require recompiling the app; the report
separately hashes the harness. Rebuild after source edits; a dirty build is not an exact
committed-baseline acceptance run. No release/install binaries are staged here.

Each local owned run retains `report.json`, private profile, generated fixtures
and window-only failure artifacts. Reports distinguish PASS/FAIL/BLOCKED,
NOT_RUN/DEFERRED/NOT_CONFIGURED and explicitly list excluded acceptance. A PASS
means only the declared cases in the selected suite, not release signoff or
acceptance of a different suite/provider scope.
Authorized-plan preflight failures before run creation print a structured BLOCKED
report, not passing tests or an owned artifact path.
The runner and provisioning helper use process-local umask `077`, inherited by
owned child processes. New local run/profile/artifact directories require `700`
and regular files require `600`. A metadata-only post-teardown audit checks owner,
type, modes, hard links and budgets inside exactly this UUID run. AT-SPI's Unix
socket can have mode `777`; its audited `700` ancestor directories prevent other
users from reaching it. This exception does not relax regular-file permissions.
The exact opt-in link suite additionally audits captured leaf-link inode/device,
spelling, owner and owned referent state, and requires exactly two known hard-link
aliases. Symlink mode 777 is confined below the private generated-data directories;
metadata/credentials remain private regular single-link files. Link-policy runs
are never eligible for automated cleanup.
Private profiles can contain OAuth tokens; securely handle retained runs.

New runs register an ownership nonce and approved-local-root hash in private,
ignored `target/native-test/.retention` metadata. Preflight reserves 128 MiB before
creating an owned run. Limits are 96 registered runs and 640 MiB of accounted
local data/reservations, with 128 MiB, 10,000 entries, depth 32 and a checked
10-second elapsed budget per local tree audit. These are fail-closed accounting
guards, not filesystem quotas or hard deadlines for stalled filesystem calls.
Remote fixture bytes and older unregistered runs are outside this accounting;
no approved-root inventory is performed. An uncertain audit blocks new runs.

The NT3 progress suite uses at most 64 KiB per generated file and exact,
one-use candidate-only checkpoints. A checkpoint holds real finalization for
1.8 seconds; selected local stream writes report actual 16 KiB increments
and pause for 250 ms. Checkpoints accept only generated source/destination
children, at most 32 plans, five seconds per hold and 500 ms per write pause.
They neither invent transferred bytes nor run in production builds. Cloud
readiness may wait 180 seconds and operation completion 600 seconds. These
small fixtures do not establish large-file throughput or every provider's
mid-file callback cadence. The registered-run limit increased from 64 to 96
to retain NT3 reports alongside prior evidence; byte and audit limits remain
unchanged, and no retained recovery data is deleted.

`--suite cancellation` adds real Cancel-button checks on every hub route before
writing, a representative local mid-file stop and local/cloud between-file stops.
Exact-path validation/write checkpoints hold at most five seconds and respond
to the real cancellation token. Two independent readbacks after quiescence
check preserved sources and truthful retained/rolled-back/completed outputs.
Read-only inspection checks live Tauri callbacks; candidate status checks the
actual task registry. Mid-file GIO/cloud timing is outside this native scope.

`--suite overwrite` verifies distinct old/new bytes on interrupted or denied
file overwrites. Local/USB mid-file cases inspect exact protected originals and
recovery diagnostics under only the candidate's private `undo-sessions` store.
Pre-write cloud cases verify original targets in both directions. Source-denial
fixtures keep the existing 4 KiB descriptor-helper bound. This suite does not
claim cloud/GIO mid-file rollback or transactional directory replacement.

Successful local-only runs must be retained for at least seven days before they
can become eligible for explicit cleanup. Age never triggers automatic deletion.
Failed, blocked, active, incompletely torn-down, legacy/unregistered and
multi-provider runs preserve their data, including the local ownership anchor
for remote recovery. This increment supplies no remote cleanup command.

```bash
node frontend/e2e-native/retain.mjs --audit RUN_UUID
node frontend/e2e-native/retain.mjs --plan RUN_UUID
node frontend/e2e-native/retain.mjs --cleanup RUN_UUID REVIEWED_PLAN_SHA256
```

Audit reads bounded metadata from the exact registered run. Plan additionally
requires a matching private schema-2 owner, schema-3 native PASS report, passing
cases, confirmed four-step teardown and expired minimum retention. The recorded
runner/driver/candidate process identities must all be gone. Runtime sockets
also prevent cleanup because sidecar process ownership has not been established.
Presence of `link-policy.json` prevents automated cleanup regardless of age/status;
all captured links and their referents remain retained for explicit manual review.
Cleanup rechecks
the plan hash and each entry's identity, then unlinks entries in postorder,
preserving owner/report metadata until other data is removed. It never deletes
the approved root, adopts a legacy run or signals a process. Changed plans reject
before deletion; an identity/I/O failure during deletion records uncertainty and
stops without retry, potentially leaving a partial tree.

For a specifically identified uncertain run, `--review RUN_UUID` can restore
byte accounting after private-tree checks, confirmed teardown and proof that all
recorded processes have exited. It clears only a matching inactive marker and
updates the registry; the original report/status and all recovery files remain.
It cannot adopt missing/legacy ownership or repair non-private data. Concurrent
editing remains outside the trusted-session guard described above.

Commit source, shared tests, example config, docs and dependency lockfiles. Do not
commit machine approvals, credentials, binaries, raw reports, screenshots/logs or
fixtures. `.gitignore` covers local configs, owned run names, `ai_agent_testfolder`
contents and build artifacts. Review `git diff --cached` before committing;
ignore rules do not remove already tracked secrets. Publish only a redacted,
scope-specific acceptance summary in the existing daily-driver run records.

## Initial implementation status (2026-10-05)

Policy/orchestration tests are separate from native acceptance. On this machine,
preflight initially found the local root but no USB root at the approved path,
no WebDriver binaries and no cloud test config. Exact network/mobile GIO metadata
queries succeeded while their reported local GVFS paths did not exist. No personal
folders were searched. These are blocked acceptance scopes, not checked daily-driver
rows. Resolve environment/access prerequisites before running those providers.

Subsequent metadata-only checks identified a missing session FUSE bridge despite
working GIO mounts. The backend recovery smoke test started the installed bridge
and verified both explicitly approved network/mobile test directories through
canonical local paths. This confirms path access only, not native UI or
file-operation acceptance for those providers.

The `transfers-pairs` suite explicitly tests all twenty ordered cross-provider
routes (forty copy/move mixed-tree parts), including the local hub again.
Each part retains bounded, sanitized routing receipts from its owned candidate's
private INFO log as well as independent exact source/destination readback.
Dispatch alone is not success; no separate Browsey staging tree is allocated for
these clipboard operations. Provider-internal temporary files are outside this
receipt claim. Missing/unknown routing evidence blocks acceptance.

`--suite moves` verifies before-write cancellation within all five providers
and across the local hub, with a representative USB mid-file stop and USB/cloud pre-delete,
between-file and partial-failure cases. A cloud failure is one exact-source
candidate dispatch fault; no shared provider/service is stopped. Both generated
trees, remaining cut selections, truthful counts and released registries are
checked. Complete late-cancel destination copies may remain alongside sources;
this is cancellation, not a promise of transaction rollback.

`--suite access` reuses real Properties inspection on all five providers and adds
actual local read/write denial and read-only source copy. Restrictive modes are
changed/restored through original no-follow descriptors, only for matching
owned inodes. Unsupported provider chmod/chown controls remain absent or disabled;
no mount-wide restriction or ownership change is performed.

`--suite iofaults` declares five bounded I/O failures: no space before writing
and after one actual 16 KiB write, transient I/O after one actual write, and
unavailable-provider failures in both local/cloud directions. One-use faults
match exact owned source/target pairs in the native candidate only. No physical
media is filled and no shared provider is stopped. Four additional native parts
cancel copy/cut during a bounded cloud destination-metadata delay, before the
first mutation task exists. Metadata and controls remain real; only the owned
candidate delay is instrumented. Exact trees are read twice after reply, retained
prefixes and partial counts are checked, and callback/task registries must clear.

`--suite races` uses synchronous owned writers while an exact candidate
checkpoint holds work: same-size edits during local copy and USB move, local
source removal, a local source symlink swap, a late local overwrite collision,
and a same-size edit after cloud upload before move-source deletion. Helpers
use original no-follow descriptors, bounded 64 KiB files and captured inode
checks; a temporary symlink refers only to another generated file. The writer
restores only its own unchanged link after the transfer reply. Externally removed
fixture sources are explicitly expected absent; unrelated sources remain intact.
Old overwrite recovery bytes, concurrent destination bytes and retained uncertain
output must survive. This is representative detection, not atomic exclusion of
all writers or provider compare-and-swap.

`--suite interruption --targets local` interrupts only the captured candidate
with SIGKILL after an actual 16 KiB overwrite write. Other provider plans are
rejected before fixture creation; no provider subprocess is interrupted. The
same private profile is deliberately restarted once, after confirmed owned
session/driver/candidate teardown. Unconfirmed closure blocks restart. Sources,
partial target bytes, old protected backup/marker, idle startup, empty new-session
undo/redo and real Settings recovery diagnostics are checked independently.
There is no operation replay, automatic resume, disk-power-loss simulation or
control of the installed application.

## NT4 bounded names and data cases

`--suite names` exercises eight generated special-name files and one non-empty
folder on local/USB/network/cloud; mobile uses seven supported names and an
actual UI rejection of a double-quoted name: spaces, Norwegian characters, emoji,
combining Unicode, both quotes, literal `#`, `%26`, `&`, `_`, dots and hyphens.
List/grid, special-folder navigation, batch copy/move and renaming an existing
emoji source use real controls. Exact membership and text bytes are independently
compared. Emoji is seeded; native emoji text entry remains excluded. Local Unix
creation and rename additionally preserve valid leading/trailing whitespace;
remote whitespace acceptance is not inferred. Cloud readiness allows 180 seconds
and transfers 600 seconds, with no mutation retry.

NT4 retains the existing 64 KiB fixture cap and all per-run/audit limits. With
77 prior reports accounting for 383,956,256 bytes, the total retained/reserved
budget increases once from 512 to 640 MiB to accommodate five new parts and
diagnostic runs while preserving prior evidence. No automatic cleanup or
large-file/performance test is authorized by this adjustment.

The [NT4 name and data record](operations/linux-release/runs/2026-10-06-native-name-data-edges.md)
records the accepted provider scopes, explicit MTP quoted-name restriction and
retained earlier attempts. NT4-1 covers 29 unique parts in two candidate runs.

`--suite limits` runs reserved-name outcomes, explicit 512-character-name
rejection and invalid rename-leaf rejection on all selected providers. Local
Unix cases distinguish a 255-byte UTF-8 name from 256 bytes, reject an over-limit
4096-byte pathname below 20 owned levels, and reject unsupported filename bytes
in actual listing/search. One literal invalid byte is seeded only below an
owned local parent; after UI observation, unchanged bytes/inode are retained
under a UTF-8 name with original-name hex recorded. No recovery is discarded and
no unsupported name is silently converted into an actionable alias.

`--suite contents` drives copy/move on the nine local-hub routes with zero-byte,
one-byte (`ff`), 4097-byte and 64 KiB generated binary files. The shared raw-byte
reader uses no-follow local descriptors or bounded private-config rclone cat
without UTF-8 decoding. Both whole sides and unrelated sentinels are compared
byte-for-byte and by SHA-256; successful moves remove only requested roots.
At most two reads run concurrently. Tree verification defaults to 128 entries
and depth eight, with explicit hard ceilings of 256 entries/depth 16 for named
cases. All reads and metadata stay inside the owned run. Large files, sparse-file
remote performance and broader binary route certification are excluded.


`--suite trees` starts with a reduced six-entry tree on every selected provider,
then verifies a combined 25-entry empty/deep/wide shape (maximum depth eight),
actual recursive search and normal listing/F5 response. Local-only virtual
listing selects first/last rows, copies all 200 one-byte files and compares both
whole byte/digest sets. Each stage checks released task/callback registries.
Independent verification remains below 256 entries/depth eight; native dispatch
traversal is limited to 4096 entries/depth 32. This is not an unbounded device
stress or throughput claim.


`--suite links --targets local` verifies list/grid rendering of two exact relative
leaf links (one broken), explicit real clipboard rejection of each, independent
hard-link copy and an alias move preserving the original inode/two known names.
Referents and all approved alias slots stay below this run's local `files/`.
The native IPC plan allows only those exact local paths/spellings; unknown links,
parent links, changed/broken referent state and extra hard aliases remain refused.
A private captured `link-policy.json` supports no-follow post-teardown auditing
without locating/following outside aliases. Regular fixture APIs retain their
single-link/no-symlink policy; the narrow hard-link reader validates known inode,
owner and alias count before bounded byte reads. USB/network/cloud/mobile link
behavior is deferred; directory links/outside referents are not authorized.


## NT5 provider scope

`--suite usb --targets local,usb` reuses authoritative foundation and Properties
cases and checks supported real Unix access denial on the selected USB. Exact
owned-path statfs/findmnt records its filesystem; no formatting is enabled.

`--suite network --targets local,network` reuses foundation, progress and Cancel
cases, and adds one exclusive generated **32 MiB** deletion fixture. Only this
network deletion writer exceeds the normal 64 KiB cap: sequential 64 KiB blocks,
60-second checked elapsed budget, no sparse/truncate operation, and partial data
retained on failure. No content read/download is allowed for this fixture. Native
permanent deletion and its cancelled confirmation are independently checked by
owned-parent membership, sentinel bytes and unchanged private undo metadata.
The source-bound GIO delete implementation uses metadata enumeration and delete,
without opening file contents. This is no throughput or service-loss test. All
normal fixture, read, retention and audit caps remain unchanged. Stale-path rename
uses one externally renamed generated source with independent byte preservation.

`--suite mobile --targets local,mobile` reuses foundation and mobile special-name
cases. Three tiny generated 128x96 RGB PNGs are the only thumbnail inputs. A
read-only MutationObserver records grid order during cold thumbnail completion
and after adding a late image through the bounded fixture writer. Actual F5
refresh and view controls expose new entries; loaded native cache images must
decode with non-zero dimensions. Complete grid samples preserve Name ascending
order. File Properties matches independent PNG size/type/name, and every source
image is verified byte-for-byte/SHA-256 afterwards. Recorded timings describe
this mounted device, not a throughput threshold or locked-phone acceptance.

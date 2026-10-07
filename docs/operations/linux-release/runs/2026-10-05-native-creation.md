# Native creation verification

Date: 2026-10-05. Scope: NT1-4 in the
[native suite TODO](../../../todo-archive/TODO_NATIVE_TEST_SUITE.md). The separate
creation suite uses real WebKitGTK and Rust inside exclusive owned runs.

## Cases and fixes

Each provider case records 24 parts. The list tests create an empty folder with
Enter and a zero-byte file with Create, cancel both dialog kinds with Cancel and
Escape, and reject empty/whitespace names, slash/backslash names, `.` and `..`,
existing file names and existing folder names for both kinds. Grid tests correct
a rejected file draft in the same dialog and create another empty folder with
the button. Modal opening checks default value/input focus; closing checks that
focus returns to the original collection. Escape follows the shared behavior:
first blur a focused text field, then close with another Escape.

Independent verification runs after every creation, cancellation and rejection.
The base folder must contain exactly the expected paths and kinds. Existing
sentinel bytes, nested sentinel bytes and the existing directory's exact listing
must stay unchanged. Newly created files must have empty contents, and newly
created folders must have no children. Expected rejection text is checked inside
the still-open modal; unexpected errors are not treated as successful idle.

Source inspection found that folder creation did not validate cloud leaf names
before joining them to a path. Both file and folder services now validate trimmed
leaf names consistently before local or cloud IPC, rejecting empty names,
separators, NUL and dot components. A regression initially failed before the fix,
and now verifies no mutation command is sent for invalid names. Native rejection
therefore exercises production validation, rather than relying on the test guard.
NUL is covered by unit tests; native keyboard entry excludes it.

Interactive cloud folder creation previously used the provider's idempotent
mkdir directly. The command now probes the exact target first and rejects either
an existing file or directory with DestinationExists. The provider's mkdir keeps
its existing ensure-directory behavior for transfer/setup callers. Cancellation
is checked before the probe and again before mkdir. Fake-provider regressions
verify preservation, no mkdir on collision, one mkdir for a new folder and no
provider I/O for a request that was already cancelled. This probe does not claim
atomic exclusivity against a competing provider writer between stat and mkdir.

The native run also exposed the old collision error remaining visible while a
corrected creation request awaited cloud I/O in the same dialog. Both creation
controllers now clear the stale error when starting a new valid request, before
awaiting the backend. Modal regressions cover the pending corrected request,
duplicate in-flight suppression and successful closure. New rejection errors
still appear normally; the harness does not suppress them.

Preservation checks use at most two independent read-only fixture operations at
once. Each pair waits for both results even if one fails, so a fixture child
cannot outlive failure recording/teardown. The same membership and byte checks
run after each attempt; a regression verifies this failure-wait behavior.

## Automated verification

All 657 frontend tests passed; type checks found zero errors/warnings and frontend
lint passed. Rust passed 770 tests with 19 ignored; clippy passed with warnings
denied. Creation orchestration tests cover declared provider/part scope, corrupt
existing bytes after acknowledged rejection, and a cancellation failure without
retry or falsely passing unfinished providers. Native policy/lint verification
passed all 83 tests and covers the Norwegian slash input action. Native lint
passed, and strict documentation consistency passed all 20 checks.

## Native evidence

Baseline: `730d4d53d562570773ab2017fc412a6cfeebec89`, with uncommitted application
and harness changes (`dirty=true`). The initial debug native-test candidate used
for the first two runs was built at 20:52:06 UTC.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `a10303b827e87e9cb5599943509638625e3f842b45cd836241297e3fab190ce6` |
| Build inputs | `cfda220b061e376839a2eac627f636ea0fab68de9e7b58c769a5b6f9517270bb` |

The first run, `792fb218-f3e4-4db6-8147-86525f8e8884`, 20:52:28–20:52:48 UTC,
is BLOCKED. Six local parts and owned accessibility passed before the driver
typed `invalid7name` for the intended `invalid/name` on Norwegian input. Exact
value checking stopped before submission. Later providers remained NOT_RUN.
All four teardown stages passed; independent checks confirmed recorded processes
gone, and the fresh private audit observed 5,022,038 bytes and 58 local entries.
Its harness hash is `573ece3b4e276d6d941875c7888921e439ee6992efdf63332df2540094cf9fe1`.
This report and recovery data remain retained; it is not reclassified by a retest.

The maintainer-approved temporary US change was attempted through runtime
Hyprland settings. The driver's main keyboard kept its own Norwegian keymap,
so no US native run was started. Global and per-keyboard Norwegian layouts were
restored and checked; no configuration file was edited. Runtime commands follow
the [Hyprland config API](https://wiki.hypr.land/configuring/core/config-options/)
and [per-device API](https://wiki.hypr.land/configuring/core/devices/).
The harness instead explicitly presses/releases Shift+7 for slash when its
recorded layout is `no`; other layouts retain their normal slash action. Exact
text/focus checks still precede submission, and a policy regression covers both
action branches without claiming native acceptance of two layouts.

The second run, `1d60d71b-6dd1-4607-84ac-0d6800a3e8e7`, 20:56:25–21:06:24 UTC,
is FAIL. All 72 local/USB/network parts, 22 cloud parts and owned accessibility
passed. The corrected-draft part observed the old DestinationExists error while
the new request was pending; mobile remained NOT_RUN. This led to the stale-error
fix above. The run used the same initial candidate/build inputs, with harness
hash `35a4d464b41d03147518e107d1eb03b6ce191e263c067a8eb7dcf07f64dadc66`.
All four teardown stages passed and independent checks confirmed recorded
processes gone. Its fresh private audit observed 5,046,142 bytes and 75 local
entries. The FAIL report and all recovery data remain retained.

The final run, `65d19ea6-388d-401f-ad32-03ecb7283aa4`,
21:09:35–21:18:40 UTC, is PASS. The candidate was rebuilt at 21:09:03 UTC after
the stale-error fix, on the same baseline with uncommitted changes. Current
application inputs and harness hashes independently match this accepted run.

| Final evidence | SHA-256 |
| --- | --- |
| Candidate executable | `fcbb361d24cef91b8172b4ccd029bba834000b9f3c186eba061faee5c446aaa3` |
| Build inputs | `6b1104f3df121ee287a2ebd920a3e93ecd2eeec4796d219ecda9f813a1434a44` |
| Harness | `47997c93f9af1bcd620e567c52aeaf72e4c5260646e6c41b5b10c4bef466f765` |

| Provider | Creation parts | Independent preservation |
| --- | --- | --- |
| Local disk | 24/24 PASS | PASS |
| USB | 24/24 PASS | PASS |
| Network/GVFS | 24/24 PASS | PASS |
| OneDrive/rclone | 24/24 PASS | PASS |
| Mobile MTP/GVFS | 24/24 PASS | PASS |

All 120 creation parts, owned accessibility, setup and all four teardown stages
passed. Independent checks confirmed all three captured processes gone. This ran
on Linux x64, kernel 7.2.5-3-omarchy, Node 26.10.0, GTK 3.24.52 and WebKitGTK
2.52.6, using Norwegian input and the owned candidate fullscreen at 1920×1080.
After teardown the global and every device layout remained Norwegian; compositor
configuration errors were empty.

The final fresh private audit observed 5,000,894 bytes and 74 local entries.
The registry holds 25 runs, all RETAINED, accounting for 114,831,225 local bytes.
Earlier BLOCKED/FAIL reports and all provider recovery data remain retained.
These byte counts cover registered local data, not remote provider storage.
NT1-4 is checked only after this complete five-provider acceptance and audit.

## Production build after commit and push

Application fixes, the native creation suite and accepted evidence were committed
and pushed as `8fbb0b7401d69d8ef057aeff6ab1dbecf0dc6c6f`. The clean checkout then
built successfully with the optimized release profile using:

```bash
PATH=/absolute/path/to/rust/bin:$PATH frontend/node_modules/.bin/tauri build --no-bundle -- --locked
```

The executable was completed at 21:27:13 UTC. `target/release/browsey` is
29,348,256 bytes with SHA-256
`71e285c3e10c2dfcb566523454501c20970cafb9e28ab8c6d8c79c0bc1f87ff9`.
The fresh release Cargo fingerprint has `features=[]`, excluding `native-test`.
Build inputs match the final accepted candidate's source hash above. This is
successful build evidence; the Git-ignored binary was not installed or launched,
and this does not claim installed-build or release signoff.

## Limits

This is generated small-fixture creation acceptance. Concurrent provider writers,
permissions/disconnect fault injection, platform-specific reserved names, NUL
keyboard input, other keyboard layouts and installed-build/release signoff remain
outside this run. Watchers and device discovery/mounting remain disabled. No
personal application is controlled and no real retained run is deleted.

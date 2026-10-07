# Native selection verification

Date: 2026-10-05. Scope: NT1-3 in the
[native suite TODO](../../../todo-archive/TODO_NATIVE_TEST_SUITE.md). The new selection
suite drives real WebKitGTK and Rust in the isolated native candidate.

## Cases and independent verification

Each selected provider runs 12 parts: six in list and six in grid. They cover
single selection, Ctrl addition/removal, Shift ranges, shrinking a range with
Shift+arrow, ordinary arrow selection, empty-space clicks, Escape, navigation
into an empty folder and restoration on Back, exact subset copy, and select-all
within a text-filtered listing. The filtered selection excludes the directory.

The UI checks each rendered entry's selection class and the statusbar's file and
folder counts. Each copy and paste shortcut is submitted once, with explicit
clipboard/result acknowledgement before proceeding. Independent shallow target
listings must contain exactly the intended unique files, and every file's bytes
must match its generated source. Source readback confirms preservation.
These observations are not backend invocation receipts; repeated requests,
context-menu operation dispatch and deletion remain NT1-6 scope.

A separate local case uses 200 generated files. It proves that the first and
last windows render fewer entries than the complete folder, moves to the last
entry using arrows, changes a range and toggles selection there, clears with
Escape, then selects and copies all 200 entries. Independent membership and byte
checks cover every destination and source, including entries outside the rendered
window. This representative list test does not certify large directories or
virtualized grids on every remote provider.

The fixture snapshot retains its default 32-child bound. Large generated tests
can explicitly choose a bound up to 256; outside paths, symlinks, invalid limits
and oversized results remain rejected.

## Retention capacity

The preceding run had filled the 20-run count limit. Continuing the authorized
suite increases the finite limit to 40 registered runs while preserving the
512 MiB total budget, 128 MiB per-run reservation/audit limit, entry/depth/time
budgets, private ownership checks and uncertainty blocks. Before the new run,
all 20 records were RETAINED and accounted for 89,819,648 bytes. No real test
data was deleted or adopted. A synthetic regression verifies preservation of
existing records and rejection at the new count limit; byte limits are still
tested independently.

## Automated verification

All 78 native policy tests passed, including scope declarations, exact-copy
membership/corruption checks, modifier release after pointer failure, outside
selection rejection, submitting one acknowledged copy/paste pair, and stopping
after one unacknowledged clipboard request.
The 36 relevant frontend tests passed across file-action orchestration, shared
selection actions, grid keyboard handling and input handlers. Native lint passed.
The strict documentation consistency check passed all 20 checks.

## Native evidence

Baseline: `e740a500769731f821b32e0a24b55006bba0524a`, with uncommitted harness
changes (`dirty=true`). The debug native-test candidate was built at 20:37:22 UTC.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `070d4e2e39960e11c6be78b6b36f433cb35332d634cdc87df46b8383333d539c` |
| Application build inputs | `934ccdb48f709c465d9c5b957056099f52b6c841ffb451732737050ac72ab255` |
| Five-provider harness | `3ca3625ec8a8db1a313518849b2decb96e98e265ad000732aa2b01b717a8faca` |
| Final local-retest harness | `a496f4993aa4deda3b6757d2100b144127742c874886233f4e4f564c645dfe70` |

```bash
BROWSEY_CARGO=/absolute/path/to/cargo bash scripts/dev/test-native-linux.sh --build
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --suite selection --fullscreen --a11y
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --suite selection --targets local --fullscreen --a11y
```

Run `767a181b-dd88-4207-9298-16119794feb4`, 20:37:47–20:44:37 UTC, is overall
PASS: five provider cases with 12 parts each, five local virtualization parts,
and owned AT-SPI accessibility. All setup and the four teardown stages passed.
Independent checks confirmed the candidate and both drivers were gone. Its
privacy audit passed; a fresh bounded audit observed 4,988,821 bytes and 482
entries in the exact local run.

The final harness corrects virtual-case requirements so they declare only the
operations used by that case, adds selection to CLI help, and records copy UI
acknowledgement before independent verification. It does not change selection
or transfer behavior in Browsey. A fresh local run verifies these reporting
changes without repeating remote mutations.

Run `6a415dbe-90d7-4a01-908f-e7ce14e9cfd5`, 20:45:52–20:46:17 UTC, is overall
PASS: all 12 local list/grid parts, all five virtualization parts and owned
accessibility. Other configured providers are explicitly DEFERRED in this run;
their acceptance comes from the preceding five-provider run. All setup and four
teardown stages passed. Independent checks confirmed all three processes gone,
and the fresh private audit observed 4,953,682 bytes and 475 entries. The recorded
harness hash matches the final implementation.

| Provider | List/grid selection parts | Exact copy readback |
| --- | --- | --- |
| Local disk | PASS, 12/12, including fresh retest | PASS |
| USB | PASS, 12/12 | PASS |
| Network | PASS, 12/12 | PASS |
| OneDrive | PASS, 12/12 | PASS |
| Mobile | PASS, 12/12 | PASS |
| Local virtualized list | PASS, 5/5, including fresh retest | PASS, all 200 files |

Host: Linux x64, kernel 7.2.5-3-omarchy, Node 26.10.0, GTK 3.24.52 and
WebKitGTK 2.52.6. Both owned fullscreen candidates were 1920 by 1080; Norwegian
input was recorded. Desktop configuration was not changed. All 22 registered
real runs remain RETAINED, accounting for 99,762,151 local bytes; none is uncertain.
Private reports, logs, credentials, machine paths and binaries remain Git-ignored.
NT1-3 is checked for this declared scope, not as release signoff.

## Build scope and limits

This increment changes the test harness and documentation only. Application
build inputs remain identical to the accepted recursive-search implementation.
The separately scoped native candidate was rebuilt for the current baseline;
the normal production binary does not need another build for these tests.
No installed or personal application is controlled. Norwegian input and the
previously authorized fullscreen mode apply only to the owned candidate.
Watcher behavior, device discovery/mounting, disconnect/reconnect, other keyboard
layouts, large-file transfer performance and release signoff remain excluded.

# Native listing and recursive search verification

Date: 2026-10-05. Scope: NT1-2 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). Shared cases cover
local disk, USB, network, OneDrive and mobile in a guarded native candidate with
real WebKitGTK and Rust. Fixtures and reads remain inside exclusive owned runs.

## Behavior and findings

Each provider case records 15 parts: a known owned baseline; name, type, size and
modified sorting in both directions; combined name buckets; combined type, size
and modified filters; empty combinations and individual resets; hidden files and
grid reset; sort preservation when switching views; case/extension text filtering
in list and grid; scoped recursive search; search view/draft transitions; and
searching an empty folder before returning to the same parent.

Independent shallow metadata supplies expected file order and verifies exact
fixture sizes. Final byte readback checks all eight generated files per provider.
The nested matching file must appear in recursive search, while a matching file
in a sibling directory must not appear. Exact entry sets, current path and view
are checked throughout; returning from search must clear its draft/filter state.

The search worker previously treated rclone URIs as filesystem paths and fell
back to the home directory when a start directory did not exist. Search now
requires an explicit valid start directory and reports a missing directory or
file-as-root instead of changing scope. Cloud URIs take a separate provider path.

Following the maintainer's requirement, cloud search is recursive. It reads live
directory metadata through the existing rclone provider, applies the shared query
language and preserves capabilities, starred state and filter facets in streamed
results. It does not download file contents. A search checks provider-returned
child paths before emitting matches or scheduling subdirectories. Read errors
retain prior streamed results and finish with an error; cancellation suppresses
stale results/completion. Provider operations use the shared remote limiter and
existing cancellable reads with bounded per-request timeouts.

The native IPC guard now permits search only with an explicit owned root. Local
trees keep the existing symlink inspection; cloud traversal verifies every child
against its current directory. No implicit home or outside-root search is allowed.

The native retest exposed stale results when editing a submitted search without
Enter. Svelte's reactive call did not explicitly depend on input/mode changes,
and cancelling the worker alone did not clear completed results. The page now
passes both reactive values to the session helper. A changed draft invalidates
the worker and clears entries, errors and facets while preserving the folder,
search mode and column filters. Only a new submission starts another search;
late chunks/completion cannot repopulate the old results.

## Automated verification

The full Rust suite passed 768 tests with 19 ignored. Search regressions cover
missing/file roots, cloud URI routing, recursive traversal through nonmatching
directories, metadata/facets, starred matches, malformed outside paths, provider
failure without retry/success completion, cancellation, duplicate suppression
and bounded result chunks. Clippy passed with warnings denied.

All 654 frontend tests and 71 native policy/orchestration tests passed. Type
checks found no errors or warnings, and frontend/native lint passed. The listing
orchestration model independently checks filters/search/sort transitions and
readback corruption, preserving prior parts and unrun providers after failure.
Fixture metadata checks reject symlinks, outside paths and more than 32 children.
These tests supplement native acceptance and do not replace it.

## Native evidence

Baseline commit: `9353a00a62383e453fdd8582e144ac48f079f849`, with uncommitted
application and harness changes (`dirty=true`). After the stale-results fix,
the separate debug candidate was rebuilt at 20:16:37 UTC with the opt-in
`native-test` feature.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `6792c8ef63e61fd5e41ce963b11488953952813d99122adb32328b0c887182ab` |
| Build inputs | `934ccdb48f709c465d9c5b957056099f52b6c841ffb451732737050ac72ab255` |
| Listing harness | `0e1bf3a0d5724b416261cc6a55a02c5d8455bd0e66fba8f34134cee05ab16c14` |

```bash
BROWSEY_CARGO=/absolute/path/to/cargo bash scripts/dev/test-native-linux.sh --build
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --suite listing --fullscreen --a11y
```

The first run, `3d6c7370-fb4f-48f6-bea9-a81ee4a33d13`, started at 20:11:31 UTC
and finished at 20:11:48 UTC with BLOCKED. Its first ten local parts and owned
accessibility passed; later providers stayed NOT_RUN. The harness used Backspace
after selecting the one-character text filter, which intentionally exits filter
mode and blurs the input. The next uppercase query consequently started in the
wrong input mode. Nonempty replacement now types directly over the selection,
preserving filter mode, and exact input/focus assertions remain mandatory.
A regression covers this interaction. No query or case is automatically retried.
All four teardown stages passed, independent checks found recorded processes
gone, and its private retention audit passed. Its report/fixtures remain retained.

The second run, `6d445d56-c89e-45c7-9fd4-fe6994726ca8`, started at 20:12:57 UTC
and finished at 20:14:17 UTC with BLOCKED. Its first 13 local parts passed,
including both text-filter views and scoped recursive search; the draft-change
part timed out with the old matches still visible. The screenshot and source
inspection identified the stale-results defect described above. Later providers
remained NOT_RUN. All teardown stages, independent process-exit checks and the
private retention audit passed. This earlier report and its fixtures are retained.

Both earlier runs used the candidate built at 20:11:12 UTC: executable SHA-256
`0abcd412b794c860fd8100a278997534f5f2b88b6da0618209d49725869812eb`, build-input
SHA-256 `23215bd46949158184ed9853c34b4c3dd5c3c621d5c5b4458b080c887533f4fd`.
They are separate blocked evidence and are not reclassified by a later retest.

Run `f04cda9a-d01d-497d-a67c-7c1cc11a0b92` started at 20:17:24 UTC and finished
at 20:21:18 UTC with overall PASS: all five listing cases, all 75 recorded parts
and the owned AT-SPI accessibility case. All setup stages passed. Session close,
driver exit, native-driver exit and candidate exit passed; independent process
checks confirmed all three recorded PIDs were gone. The private audit and a fresh
read-only retention audit passed. All 20 registered real runs remain RETAINED
with no uncertain accounting state; no recovery data was deleted.

Host: Linux x64, kernel 7.2.5-3-omarchy, Node 26.10.0, GTK 3.24.52 and WebKitGTK
2.52.6. Norwegian input was recorded. The authorized fullscreen helper verified
only the owned candidate at 1920 by 1080; desktop settings were not changed.

| Provider | Listing/filter parts | Scoped recursive search | Independent byte readback |
| --- | --- | --- | --- |
| Local disk | PASS, 15/15 | PASS | PASS |
| USB | PASS, 15/15 | PASS | PASS |
| Network | PASS, 15/15 | PASS | PASS |
| OneDrive | PASS, 15/15 | PASS | PASS |
| Mobile | PASS, 15/15 | PASS | PASS |

NT1-2 is checked for this declared scope, including recursive OneDrive search.
Private reports, credentials and machine paths remain Git-ignored. The installed
or personal application was not controlled. The two earlier BLOCKED reports
remain separate evidence.

## Production build after commit and push

Application changes, the native listing suite, regression tests and acceptance
evidence were committed and pushed as `81c90316b70b48b0457fe9693e4688ba9488a5f6`.
The subsequent normal production build uses the optimized release profile:

```bash
PATH=/absolute/path/to/rust/bin:$PATH frontend/node_modules/.bin/tauri build --no-bundle -- --locked
```

The build completed at 20:26:30 UTC. `target/release/browsey` is 29,347,744 bytes
with SHA-256 `a2f83e0c18917705a498dd0573412fdd1febff72d507b00dcbe13eaef3cb6f8d`.
The latest release Cargo fingerprint has `features=[]`, excluding `native-test`.
Build inputs still match the accepted candidate's source hash. The binary remains
Git-ignored and was not installed or launched. This is successful build evidence,
not installed-build acceptance. The following commit adds only this build record.

## Limits

This is small-fixture listing/search acceptance, not release or installed-build
signoff. Watchers, mounting/disconnect/reconnect, large directories, large-search
performance, other cloud providers and other distributions remain outside this
run. Modified sorting uses independently observed timestamps at the application's
minute precision, including name ordering for ties; fixtures do not cover date
range or timezone boundaries. One Norwegian keyboard layout is sufficient.
Real recovery data is retained. The configured 20-run retention count is now
fully used; further runs must respect the bounded retention policy.

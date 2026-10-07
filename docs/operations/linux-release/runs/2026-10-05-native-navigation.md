# Native owned-folder navigation verification

Date: 2026-10-05. Scope: NT1-1 in the
[native suite TODO](../../../todo-archive/TODO_NATIVE_TEST_SUITE.md). Reusable navigation
cases cover local disk, USB, network, OneDrive and mobile in a separate guarded
Browsey candidate with real WebKitGTK and Rust. Only generated data under this
session's approved test roots is accessed.

## Behavior and findings

Each provider runs the same list and grid cases. Eight recorded parts verify an
owned bookmark and view, nested folders, back/forward, an in-scope breadcrumb,
an empty folder refreshed through the main menu, an externally created fixture
becoming visible after F5, repeated visits and view switches preserving the
current folder. Exact folder/view/entry observations accompany each step.
Independent readback checks all four generated files and preservation of four
directories per case. Saved bookmarks must match the session's owned roots.

Native testing found three harness menu problems: an unchanged view left its
overlay open, changing views already closed that overlay, and a combined CSS/text
selector was unsupported by the native driver. The harness now handles both
overlay states and uses an exact menu-scoped XPath for the Refresh button. No
keyboard mutation is retried to mask a failure.

The initial full-provider run reached OneDrive but F5 retained an empty cached
listing after independent readback confirmed the new generated file existed.
Explicit folder refresh now requests fresh provider data, bypassing both fresh
and stale listing cache entries. Ordinary navigation retains its cache behavior.
Failed or cancelled explicit refresh remains an error and preserves the previous
good cache instead of claiming successful refresh. The main-menu Refresh action
uses the same application path.

F5 remains enabled as the folder-refresh shortcut in both production and native
builds, following the maintainer's final preference. Its handler prevents the
WebView default synchronously before awaiting I/O, so slow or failed refresh
does not become a browser-page reload. Unit regressions cover both outcomes.

The maintainer authorized fullscreen for the owned candidate to expose long
breadcrumbs. The helper validates PID, executable and private profile, targets
one matching window address, and confirms fullscreen at 1920 by 1080. Personal
windows and desktop configuration are untouched.

## Native evidence

Baseline commit: `0e3f84dea0135222c0e6003e38c828fa76df3524`, with uncommitted
application and harness fixes (`dirty=true`). The separate debug `native-test`
candidate was rebuilt at 19:45:03 UTC. Host: Linux x64, kernel 7.2.5-3-omarchy,
Node 26.10.0, GTK 3.24.52 and WebKitGTK 2.52.6. Norwegian input was recorded;
the desktop layout was not changed.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `9dc8168e556e52927ff65749a29946c0a8e62717d720c12cc55b7afe605c8867` |
| Build inputs | `39df763dc881391bd9249f4f81e40034114e9b927b6c497b80dd12c4d43740ee` |
| Navigation harness | `d9f3e287f18d00268c8466bddaf26a51a2fcb97b709a7f1b751258e05e52cc14` |

```bash
BROWSEY_CARGO=/absolute/path/to/cargo bash scripts/dev/test-native-linux.sh --build
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --suite navigation --fullscreen --a11y
```

Run `ae3ecc0e-93be-4f71-adce-3dca58b15477` started at 19:45:10 UTC and finished
at 19:50:13 UTC with overall PASS: ten navigation cases, all 80 recorded parts,
and the owned AT-SPI accessibility case. All setup stages passed. Session close,
driver exit, native-driver exit and candidate exit passed, and an independent
check found all recorded processes gone. The private local-tree audit passed;
all 17 registered real runs remain retained with no uncertain accounting state.

| Provider | List | Grid | Recorded navigation parts |
| --- | --- | --- | --- |
| Local disk | PASS | PASS | 16/16 |
| USB | PASS | PASS | 16/16 |
| Network | PASS | PASS | 16/16 |
| OneDrive | PASS | PASS | 16/16 |
| Mobile | PASS | PASS | 16/16 |

NT1-1 is checked for this declared scope. Earlier blocked reports and generated
fixtures remain retained and are not reclassified by the successful retest.

| Earlier run ID | Scope and outcome |
| --- | --- |
| `cca65b74-463e-4321-8e37-047a7d6b680f` | Local BLOCKED: unchanged-view menu overlay intercepted navigation. |
| `22e11b1a-381e-489c-b648-1f41651ab879` | Local BLOCKED: view switch had already closed the overlay. |
| `723399b8-4ee0-4d95-a0fe-bd7d8c92e2b4` | Local PASS: both views, all 16 parts and owned accessibility. |
| `2075968f-2e7d-4988-a4d0-b9098311b333` | Full-provider attempt BLOCKED at OneDrive's new-file listing; local/USB/network passed both views, later cases stayed NOT_RUN. |
| `8fa55fcb-7ccc-486e-a0ab-8b23bad9203f` | Full-provider attempt BLOCKED at the first local menu Refresh: invalid harness selector. |

## Automated verification

The complete Rust suite passed 761 tests with 19 ignored; clippy passed with
warnings denied. Cloud-cache regressions check forced fetch on fresh/stale cache,
single fetch, preservation of unrelated entries, and failure/cancellation without
cached success. Frontend regressions check explicit refresh forwarding and
history preservation plus synchronous F5 default prevention.

All 650 frontend tests and 65 native policy/orchestration tests passed. Type checks
passed. Frontend/native lint and strict documentation consistency passed. These automated
results supplement the native run; they do not replace provider acceptance.

## Production build after commit and push

Application, harness, regression tests and acceptance evidence were committed
and pushed as `31d2ea7325f90bb0e90219f073e81073d20ebf30`. The subsequent normal
production build completed at 19:55:34 UTC with the optimized release profile:

```bash
PATH=/absolute/path/to/rust/bin:$PATH frontend/node_modules/.bin/tauri build --no-bundle -- --locked
```

The resulting `target/release/browsey` is 29,333,144 bytes with SHA-256
`24c67e88df755f816248d5de8b813a710c4dbc2b262bff134ff31870fc33f668`.
The release Cargo fingerprint has `features=[]`; the native-test feature is
absent. The artifact remains Git-ignored. This records a successful build,
not installed-build acceptance; the installed/personal application was not
launched or replaced. The following commit adds only this build evidence.

## Limits

This is bounded owned-folder navigation acceptance, not release or installed-app
signoff. Watchers are intentionally disabled in the guarded candidate. Mounting,
disconnect/reconnect, ancestor navigation, narrow-window layout, large virtualized
lists and other distributions remain outside this run. One Norwegian keyboard
layout suffices. Private reports, paths, credentials and screenshots remain
ignored; no real recovery data is deleted.

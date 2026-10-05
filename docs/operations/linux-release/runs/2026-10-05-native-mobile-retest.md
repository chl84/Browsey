# Native mobile foundation retest

Date: 2026-10-05. Scope: retry the exact already approved mobile test folder after
the blocked preflight in the [reporting run](2026-10-05-native-reporting.md).
The same folder was now available. Only generated fixtures in a fresh owned
local/mobile run were used; no device-root discovery, shared-service restart,
connection action or installed Browsey control was performed.

## Candidate and commands

The initial metadata check passed both approved roots. It rejected the staged
candidate's older commit, so the separate debug `native-test` candidate was
rebuilt from clean commit `9868e1298153bd3580e8ba2b9f85ed926d481601`
(`dirty=false`) at 19:00:56 UTC. No app or harness source changed.

```bash
bash scripts/dev/test-native-linux.sh --check --targets local,mobile
BROWSEY_CARGO=/absolute/path/to/cargo bash scripts/dev/test-native-linux.sh --build
BROWSEY_NATIVE_INPUT_LAYOUT=no bash scripts/dev/test-native-linux.sh --run --targets local,mobile --a11y
```

The native run used Linux x64, kernel 7.2.5-3-omarchy, Node 26.10.0,
GTK 3.24.52, WebKitGTK 2.52.6 and the same tauri-driver 2.1.0.
Norwegian input was verified; no keyboard layout change was needed.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `ace1f947bb64f6060cee12d9a916c7a6564d37e6ad0741c4ef0c9b7606b018f4` |
| Build inputs | `990431fe11805252511c2f9d89c0ea29ef924dc578ab3653dec9b40f0a9c2f49` |
| Harness | `30c87c622b2a29f152c605ec19fccbaeb406cb88a9e1147a03039b3fc465d7d1` |

## Results

The run from 19:00:57 to 19:02:02 UTC passed all 17 cases and all 16 file/directory
transfer parts. The schema 3 report, every setup stage and both selected provider
outcomes are PASS:

| Scope | Result |
| --- | --- |
| Local and mobile create/rename | New file/folder and file/folder rename passed |
| Local and mobile permanent delete | Cancel preserved data; confirmed file/folder deletion passed |
| Within local and within mobile | File/tree copy and move passed |
| Local → mobile | File/tree copy and move passed |
| Mobile → local | File/tree copy and move passed |
| Norwegian input | Exact `/`, `_`, `æøå` values/focus and independent local results passed |
| Local copy undo/redo and AT-SPI | Both passed |
| Owned teardown | Session closure, tauri-driver, WebKit-driver and candidate exit passed |

Independent checks verified generated destination bytes and source preservation
for copy/removal for move. Current owned parent-directory membership avoids stale
MTP metadata aliases. Local has 12 relevant passing cases and mobile has nine;
the four route cases belong to both. All declared capabilities for these two
provider scopes have passing case evidence. Exact captured process PIDs were
independently confirmed gone after teardown.

USB, network and cloud are explicitly DEFERRED in this report. The earlier mobile
preflight remains BLOCKED with its original NOT_RUN cases; this is a separate
fresh successful run, not a change to historical results. Generated recovery
fixtures, private profile, raw report and artifacts remain outside Git.

## Limits

Acceptance covers the existing small-file foundation and a tree containing one
generated child. It does not establish device disconnect/reconnect, progress or
cancellation, conflicts, large/deep trees, watcher behavior, other distributions
or installed-build/release acceptance. Existing daily-driver parent rows remain
open. NT0-6 retention work remains open.

Only redacted documentation changed after the run. No production Browsey rebuild
was performed afterward, following the maintainer's explicit instruction.

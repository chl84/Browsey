# Native foundation fixes and repeat verification

Date: 2026-10-05. Scope: the confirmed OneDrive/MTP findings from the
[first foundation run](2026-10-05-native-foundation.md), followed by the existing
five-provider foundation and local-hub transfer cases. Only generated fixtures
in fresh owned runs were used; the installed application was not controlled.

## Candidate

Baseline commit: `d3f540fbad3ba5db9de14645fa8526f5f9a1960d`, with uncommitted
fixes (`dirty=true`). Debug candidate rebuilt with `native-test` at
2026-10-05 17:58:56 UTC. Host, keyboard layout and driver versions match the first
run: Norwegian input, WebKitGTK 2.52.6, GTK 3.24.52, tauri-driver 2.1.0.

| Evidence | SHA-256 |
| --- | --- |
| Candidate executable | `ace1f947bb64f6060cee12d9a916c7a6564d37e6ad0741c4ef0c9b7606b018f4` |
| Build inputs | `990431fe11805252511c2f9d89c0ea29ef924dc578ab3653dec9b40f0a9c2f49` |
| Five-provider harness | `a17e9afc1f0b0bce1e4c55b9d0fcbcdb4b0ddc78590a37b98ec436ee5e5945e3` |
| Local retest harness after history-wait fix | `f9e018e5a8466dd7064cb91c4fc965a341ab0eee26c8fc2f4bd807b20624a3a6` |

## Changes

- Copy preserves file/directory permissions where supported. An explicit
  `Unsupported` response from the destination filesystem no longer discards
  copied data. Permission denial and other I/O failures still fail the copy,
  preserving the source and existing recovery behavior.
- Cloud source metadata now determines whether RC's single-file write route is
  applicable. Directories use the existing CLI `copyto`/`moveto` route before any
  RC write starts; an uncertain write is never retried to correct the route.
  This matches rclone's [file-only RC operations](https://rclone.org/rc/#operations-copyfile-copy-a-file-from-source-remote-to-destination-remote)
  and [directory-aware copyto](https://rclone.org/commands/rclone_copyto/).
- Paste remains visibly busy through conflict preview, backend work and final
  reconciliation. Child progress completion cannot hide activity before the
  command reply. The native driver observes this state and app error toasts,
  plus current clipboard acknowledgement and actual destination rows.
- Independent existence checks now use fresh parent-directory membership inside
  the owned run. MTP can retain a positive metadata lookup and old file-id mapping
  at a moved path. The verifier never lists above an owned data root.
- Fallback moves choose the existing exclusive stream writer before copying,
  including GVFS/FUSE paths. This creates a writer-based ownership receipt and
  verifies copied bytes before source removal. Ordinary GIO copies retain their
  existing route. Missing receipts, changed outputs and verification failures
  still preserve sources; no late path lookup is adopted as ownership proof.
- Paste verification observes the original request for a bounded 360 seconds,
  matching the backend's 300-second transfer deadline plus final listing
  reconciliation. Ordinary readiness waits remain 60 seconds. App errors still
  fail immediately; a timeout never counts as success or resends a mutation.
- Undo/redo verification waits for the current Undo/Redo acknowledgement and the
  reconciled absent/present destination row. An idle frame alone could precede
  history completion and send redo while Browsey was still consuming repeats.
- Mixed directory moves use rclone `move` with `--create-empty-src-dirs` and
  `--delete-empty-src-dirs`. File routes remain unchanged. The old cross-backend
  `moveto` route moved files but left empty source directories behind; its fake
  provider incorrectly renamed the entire tree. The corrected shim models
  file-by-file movement and empty-only descendant cleanup. Rclone retains its
  source filesystem root even with the cleanup flag; after destination-root
  confirmation, local `remove_dir` or cloud `rmdir` removes only that empty root.
  Local identity is captured before the transfer and checked before removal;
  changed roots, new contents, cancellation and failures retain recovery data.
  Destination root creation also covers an entirely empty moved source. See the
  [directory move contract](https://rclone.org/commands/rclone_move/).

## Verification

The earlier candidate (`7235a880ab84dce2863d8c9746a30c5d90327a46271d02f90dccaedcb836eb01`,
harness `c7b2ab01090ed752b9d91a524976f88678568a7dc41af9f41147c9746db83c70`)
five-provider post-fix run passed 26 cases, including cloud file/tree
copy/move and mobile file/tree copy, before failing the mobile move's old-path
existence assertion. Teardown passed. Subsequent read-only inspection found
only `tree` in the source directory and `sample.txt` in the destination;
metadata and even reads through the old cached file-id mapping still succeeded.
This was a verifier failure, not evidence that Browsey copied instead of moved.
The original run remains FAIL and no mutation was repeated in it.

The next local/mobile run passed within-mobile copy/move and local-to-mobile
copy. Local-to-mobile move safely refused source removal because the GIO copy
had no writer ownership receipt; both sides were retained and teardown passed.
This prompted the fallback-writer change above. A bounded, exclusive generated
writer probe inside that run confirmed MTP's FUSE path supports read/write,
sync and stable open-file identity. Its tiny fixture is retained with the run.

The earlier local/mobile candidate was built at 17:18:33 UTC, executable hash
`6684baf521103d33ed63182f243c1e1368f7fa1fdfa2839f75771c8b3726809a`,
build inputs `b456aab5b9e02dbc7a84c34ee80e7f5d0532a141beed9205a4727cdf2646d1ff`
and harness `e302790bffcea4489d44927b1af5ae63b1ee7e972ad77d834efc786a181bf08c`.
Its fresh local/mobile run passed all 17 cases. Local and mobile each passed
create, rename, permanent-delete/cancel
and file/tree copy/move. `copy-local-mobile`, `move-local-mobile`,
`copy-mobile-local` and `move-mobile-local` passed for both files and nested trees;
destination bytes and current source-directory membership were checked
independently. Input, AT-SPI and local copy undo/redo also passed. Both selected
provider results and confirmed owned candidate/driver teardown are PASS;
USB/network/cloud were explicitly DEFERRED in this bounded run.

The next five-provider run passed 21 cases (including AT-SPI) before the
within-cloud move exceeded the verifier's 60-second readiness deadline. The
failure screenshot still showed Moving activity, with the individual file
visible. Read-only inspection found that file in the destination and the tree
still in the source. Teardown passed; the run remains FAIL, and the pending
mutation was not repeated there. This motivated aligning the transfer wait with
the existing backend deadline rather than declaring early completion.

The fresh run with the aligned deadline passed 32 cases, including all five
within-provider foundations and local-to-cloud copy, before failing the
local-to-cloud directory move's source-removal assertion. The individual file
move passed; nested destination bytes also matched, but the now-empty local
source tree remained. Teardown passed and the run remains FAIL. This reproduced
the mixed-directory adapter defect described above; the original source was not
removed manually and no mutation was retried.

The next candidate (`8e0eb704a9f704932737c652a39b3fbc8bfe716c17a861ec5acaa3d3b3a3acec`,
build inputs `bdec9fff0101ae912f228d266cf0ea4b3dd9e2d56add8b1c6965e0fc39174c90`)
also passed 32 cases before reproducing the same empty local root after the
flagged directory move. It confirmed that the flag cleans descendants, not
rclone's source filesystem root. This run remains FAIL with successful teardown;
the final empty-only root-removal step above was then added.

The final five-provider run with source-root cleanup passed 43 of 44 cases.
All five providers passed their within-provider foundation and all eight ordered
local-hub routes passed both file/tree copy and move. It stopped at local undo/redo:
the driver sent redo before undo had finished its refresh and acknowledgement,
so Browsey's history busy guard consumed the shortcut. The screenshot still
showed Undo and the target was absent. This run remains FAIL; teardown passed.

Only the history-wait harness changed afterward. A fresh local-only run on the
same candidate passed all eight cases, including input, accessibility, local
foundation and acknowledged undo/redo. Its report and owned teardown are PASS;
the other four providers are explicitly DEFERRED in that report. No mutations
were retried in either run.

NT0-1 is checked for complete declared case coverage across these two independent
runs on the same binary/build inputs, not for a single all-green five-provider
report. The original full report retains FAIL. Accepted coverage is:

| Scope | Verified result |
| --- | --- |
| Local, USB, network, OneDrive, mobile | Create, rename, permanent delete/cancel, file/tree copy and move passed on all five |
| Local ↔ USB/network/OneDrive/mobile | Copy and move passed in all eight ordered routes, with destination bytes and source preservation/removal checked |
| Norwegian input and AT-SPI | Passed in the five-provider run and fresh local retest |
| Local copy undo/redo | Passed in the fresh local retest after acknowledgement/reconciliation wait fix |
| Owned candidate/driver teardown | Passed in both reports |

The five-provider run finished at 18:12:05 UTC; the local retest ran from
18:13:55 to 18:14:21 UTC. These results cover the tested uncommitted source hashes;
they are not installed-app or release acceptance.

Focused and broader checks passed:

- Rust: final full suite passed 759 tests with 19 opt-in tests ignored. The
  clipboard subset passed 94 with two opt-in tests ignored. Regressions cover
  unsupported/denied permissions, owned GVFS fallback writers, retained source
  on failed readback and the existing missing-receipt refusal.
  The updated mixed-transfer subset passed 39 with five opt-in tests ignored;
  it verifies nested/empty-directory moves and empty source-root removal in both
  directions, retention of replaced/nonempty/cancelled sources, plus partial-batch
  behavior with realistic shim semantics.
- Frontend: 647 unit tests and 107 mock-browser E2E tests passed. These are not
  native device/provider acceptance.
- Native policy/input/orchestration: 19 tests passed, including no paste before
  cut acknowledgement and immediate failure on a Browsey error toast.
  A stale-positive metadata regression verifies parent-directory membership
  and denial before any directory listing outside the owned root.
  A history regression rejects redo before undo acknowledgement/reconciliation.
- Svelte/TypeScript checks, frontend/native lint and warnings-denied all-feature
  Clippy passed. The typed-error blocking scan found no findings; the backend
  error-hardening guard passed with existing USB-format advisory seams unchanged.

An earlier post-fix native attempt was explicitly interrupted to resolve Clippy
before acceptance. Its verified owned candidate/driver exited and its private
report was marked BLOCKED. Session closure was interrupted, so it is not a PASS;
its generated fixtures were retained and not reused by the next run.

## Limits

Acceptance here is limited to the existing foundation: small generated files
and a directory containing one generated child, plus Norwegian `/`, `_` and
`æøå` input. It does not establish empty/deep/wide tree transfer semantics,
conflicts, cancellation, recovery after disconnect or other keyboard layouts.
Broader NT0-4/NT0-5 fault/capability coverage and daily-driver parent rows remain
open. Private reports, screenshots, credentials and recovery data are not
committed; old failed runs remain failures in the historical record.

# Ctrl-wheel Zoom Follow-up

Date: 2026-10-04. Baseline: `7947998`; candidate: the working-tree zoom changes
from this increment. This is development verification, not an installed release
or a direct comparison with Nautilus.

## Changes

- Remove the 80 ms wheel-step gate. Keep the existing trackpad threshold, delta
  normalization, direction/pause reset and ordinary-scroll behavior.
- Track the desired zoom separately from the rendered state. Clamp every notch
  and apply the latest target once per animation frame, retaining input received
  during an asynchronous list/grid switch. Cancel queued work on manual view
  changes, modal blocking and teardown.
- Read density styles once, calculate the final anchored grid window before
  publishing it, and restore scroll after the new spacer is committed. Capture
  list/grid transition anchors before changing the old grid geometry. Reuse the
  captured entry index when the ordering is unchanged, with a path fallback.
- Scale existing previews immediately. Defer higher-resolution thumbnail work
  until 150 ms without a size change, and let useful smaller in-flight work
  finish. First thumbnails and new-directory requests are not delayed. Offscreen,
  revision-change and navigation cancellation remain active.

## Measurements

Reuse [the existing UI workload](performance-workloads.md#browser-ui), with
synthetic images and mock IPC. Both versions ran in fresh headless Chromium
153.0.8010.12 contexts at 1536x960 CSS pixels, device ratio 1.1875, on the same
development machine. The old version used a disposable detached worktree on
port 4175; the candidate used port 4173 and the same installed dependencies.
No native app, device or cloud account was changed.

Each row contains five zoom samples; readiness includes two animation-frame
boundaries. Round 1 measured baseline then candidate; round 2 reversed the order.
No test suite or build ran concurrently with the recorded rounds. An interrupted
extra run concurrent with browser tests is excluded. These are observed medians
and maxima, not a statistically established percentile or a CI latency budget.

| Entries / round | Baseline median / max | Candidate median / max |
| --- | ---: | ---: |
| 10,000 / 1 | 88.8 / 113.8 ms | 35.3 / 50.7 ms |
| 100,000 / 1 | 91.4 / 116.7 ms | 34.6 / 39.3 ms |
| 10,000 / 2 | 67.9 / 87.3 ms | 34.7 / 56.8 ms |
| 100,000 / 2 | 67.5 / 72.8 ms | 34.7 / 40.4 ms |

Both versions rendered the same bounded grid windows: 120 or 186 cards at the
bottom, depending on zoom level, not the entire listing. The measured improvement
is consistent across the two entry counts and run orders, but native WebKit,
real image decoding, cold storage, MTP and cloud latency require separate checks.

Additional instrumented mock observations in round 2:

| Controlled observation | Baseline | Candidate |
| --- | ---: | ---: |
| Five same-task notches from list: final thumbnail size | 64 px | 192 px |
| Body / grid computed-style reads for one grid zoom | 5 / 4 | 1 / 2 |
| Thumbnail requests within three frames of that zoom | 94 | 0 |
| Five 110 ms-spaced steps with pending thumbnail work: requests / cancellations | 26 / 23 | 14 / 11 |

The last row deliberately holds thumbnail replies until the gesture ends. The
baseline requested all five intermediate pixel dimensions (76, 114, 152, 190,
228); the candidate requested only 76 during the gesture, before its trailing
resolution upgrade. Remaining cancellations concern paths moving offscreen,
not dimension-only restarts. Zero immediate upgrade requests does not mean the
eventual high-resolution thumbnail is omitted; regression coverage checks it.

## Reproduction and Coverage

Start `npm --prefix frontend run dev:e2e`, then run the existing
`measureExplorerUi(10000)` and `measureExplorerUi(100000)` helper in the
collaborative preview as documented in the workload guide. If that preview is
unavailable, use the repository's installed Playwright Chromium with the same
viewport/device ratio and invoke the helper through `page.evaluate`. Always
close the disposable browser/context, and avoid concurrent builds or suites
when comparing timings. Do not compare timings from different browser engines.

```sh
npm --prefix frontend run test
npm --prefix frontend run test:e2e
npm --prefix frontend run check
npm --prefix frontend run lint
npm --prefix frontend run build
```

Unit coverage includes rapid notches at several input intervals, frame
coalescing, clamp/reversal semantics, input during a pending view switch, modal
blocking, failure handling, teardown, one-pass anchored reflow, unchanged-order
anchor lookup, sorting fallback and thumbnail quiet-period/navigation behavior.
Browser coverage includes native Ctrl-wheel prevention of page zoom, selection,
ordinary scrolling, compact density, same-task bursts, bottom-of-folder shrink,
intermediate-resolution suppression and coalesced grid-to-list anchoring.
Numeric workstation timings are deliberately not enforced in CI.

Final checkpoint: 362 frontend unit tests, all 87 browser tests, 17 docs-site
tests, 30 release-helper tests and 20 strict documentation checks passed.
Svelte/TypeScript checking, frontend lint and the production frontend build
passed. Rust and native IPC code were unchanged; no backend benchmark or native
installation is claimed. The disposable baseline worktree and test servers
were removed/stopped after verification.

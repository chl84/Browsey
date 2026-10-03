# Disposable Performance Workloads

Date: 2026-10-03. Starting source: `fa11f5e`, plus this increment's harness and
metadata-cache changes. Environment: AMD Ryzen 5 7520U, 8 logical CPUs,
approximately 15 GiB RAM, Linux `7.2.5-3-omarchy` x86_64, home Btrfs filesystem.
These are development-machine measurements, not isolated-host CI guarantees or
a new installed release. The first measured bottleneck was local metadata
decoration/caching; removing unused local caching reduced the 100k listing
median from 840 to 537 ms in the same generated workload.

## Reproduction

```sh
bash scripts/dev/performance-workloads.sh --dry-run
bash scripts/dev/performance-workloads.sh --directory /path/to/existing/writable/parent
```

The runner uses exactly five opt-in test names with locked offline Cargo and
the optimized profile. It requires at least 1 GiB available space, generates
its own unique fixture root, isolates XDG directories and never selects all
ignored tests. It does not drop OS caches, alter network/storage settings, use
cloud accounts or install/restart the ordinary app. `--debug` is explicitly
labelled unoptimized. Production caches, recovery data and source files are not
cleanup targets. Test fixtures tear down only their uniquely created local
trees; the shell wrapper removes only an empty runner root and reports any
nonempty retained root for inspection.

Workloads reuse the production listing traversal, recursive search scanner,
thumbnail decoder/encoder/cache-hit functions, copy I/O hooks and undo engines.
The listing/search seams omit database setup, runtime admission and IPC delivery;
they do not replace the production implementation with a benchmark-only copy.
Fixtures cover 10k/100k eight-byte regular files, 100 recursive groups, simple
name search and metadata-dependent queries, eight thumbnail formats at two
sizes, repeated recovery round trips and controlled cancellation boundaries.

Measurements report five individual samples, median and maximum, not a
statistically justified p95/p99. Generated inputs already warm OS caches. A first
function call or fresh private application profile is not cold-disk evidence.
True cold/slow physical storage, USB, MTP and cloud latency remain separate
acceptance work; no global cache clearing or device manipulation was attempted.

## Local Listing and Search

Before/after listing runs used identical names, contents, sorting and fresh
fixture roots on Btrfs. The scanner extraction itself preserved existing
traversal, cancellation, query and batch semantics. Functional regressions check
sorting, starred state, fresh metadata, simple/scoped queries and cancellation.

| Core path | Entries | Before median and maximum | After median and maximum |
| --- | ---: | ---: | ---: |
| Local listing | 10,000 | 80.3 / 81.7 ms | 54.0 / 54.8 ms |
| Local listing | 100,000 | 840.0 / 862.0 ms | 537.3 / 539.8 ms |

The existing metadata cache was populated for every local file, although its
readers consult it only for network paths. Local listings now retain fresh
entry metadata without decorating/caching it a second time. Network cache
storage is bounded to 10,000 entries with constant-time insertion-order eviction;
updates do not grow a duplicate-key queue. Cache TTL and fallback semantics are
unchanged. This is best-effort display metadata, not an ownership receipt.

Process peak RSS was approximately 130.6 MiB before and 123.7 MiB afterward for
the 100k workload. This includes allocation high-water effects and the complete
listing vector; it is not retained-cache size or whole-app memory. The important
structural bound is that local paths no longer accumulate in that network-only
cache, and network entry/queue counts stay capped.

Simple recursive name search had a median around 4.8 ms for 10k files and
39 ms for 100k files, returning 10/100 matches. The name prefilter avoids metadata
work for unmatched files. Do not apply those numbers to a query requiring all
metadata or returning the whole tree. The additional `hidden:false` workload
visits/builds all entries and reports its results separately.
In a follow-up optimized Btrfs run without a concurrent build/browser suite,
`hidden:false` returned all 10,100/100,100 files/directories: median/max
67.9/71.4 ms and 717.5/736.5 ms respectively. Narrow name-search medians were
4.5/42.8 ms in that run. Broad metadata queries are therefore a different cost
class from the name-only path; no name-prefilter timing is claimed for them.

## Thumbnail Generation and Cache

Image inputs are deterministic 2560×1440 RGB patterns encoded as JPEG, PNG,
WebP, GIF, BMP and TIFF. The shared SVG/PDF paths use a simple vector rectangle
and the existing independent PDF fixture writer with text and shapes. Sixteen
owned output PNGs at 96/384 maximum dimension totalled 2,299,903 logical bytes.
This is fixture-specific cache growth, not a prediction for every user image.

| Format | Median at 96 | Median at 384 |
| --- | ---: | ---: |
| JPEG | 78.4 ms | 98.5 ms |
| PNG | 28.8 ms | 38.6 ms |
| WebP | 132.2 ms | 142.6 ms |
| GIF | 26.2 ms | 35.4 ms |
| BMP | 103.2 ms | 112.3 ms |
| TIFF | 4.6 ms | 14.8 ms |
| SVG | 0.16 ms | 0.50 ms |
| PDF | 0.76 ms | 1.87 ms |

First SVG/PDF calls include process-wide font/PDFium initialization: observed
maxima were 231/58 ms in the after run, unlike their warm medians. Cache-hit
functions return dimensions without decoding originals; their individual
median/max values are emitted in JSON. These timings exclude IPC, admission,
remote materialization and actual display. No worker-count increase or decoder
replacement was inferred from these workloads. Existing byte-budget eviction,
in-flight limits and protected-cloud-copy rules remain unchanged.

## Browser UI

Use the existing mock-IPC e2e server and the T3 collaborative preview:

```js
await import('/src/test/performanceHarness.ts').then(m => m.measureExplorerUi(10000))
await import('/src/test/performanceHarness.ts').then(m => m.measureExplorerUi(100000))
```

The helper is not imported into the production app and rejects non-e2e mode.
Use an open `/mock` folder without dialogs. It exercises real refresh, wheel
zoom and virtualized scroll handlers with bounded generated mock data, then
returns JSON rather than launching a separate automation browser.

Observed T3 Code 0.0.44, Electron 44.4.2 / Chromium 152 preview viewport:
1165×960 CSS pixels, device ratio 1.1875. Refresh/zoom readiness includes two
animation-frame boundaries. Thumbnail data is synthetic, not real filesystem
decoding. Memory is reported JS heap, not process RSS or a native WebKit total.

| Browser UI path | 10k median and maximum | 100k median and maximum |
| --- | ---: | ---: |
| Refresh with mock listing | 49.9 / 64.5 ms | 103.1 / 123.7 ms |
| Scroll to last grid entry | 42.7 / 59.2 ms | 37.2 / 46.1 ms |
| Alternating grid zoom | 60.9 / 65.8 ms | 58.4 / 61.6 ms |

Only 84–122 grid cards were rendered at either size; each run issued 168 thumbnail
requests. Reported JS heap was approximately 41.3/66.5 MiB in this session,
including previous activity and without forced garbage collection. First mock
thumbnail observations were 96/62 ms; one observation per dataset is not a tail
budget. The two lightweight browser regressions test structural virtualization
and delayed-thumbnail cancellation, without enforcing workstation timings in CI.

## Controlled Cancellation

The production manual copy test installs the existing thread-local I/O hook on
one generated 1 MiB source. At a 64 KiB read boundary it sets cancellation and
delays that read by 20 ms. Five optimized observations had median 20.35 ms and
maximum 20.38 ms from the request through worker return. Source bytes survived;
uncertain partial output remained for inspection. The latency includes the
blocked boundary; it is not proof that an arbitrary kernel/device read can be
interrupted. Search similarly stops further traversal after observing its
fixture-scoped delayed cancellation check.

## Native Candidate and Budgets

`scripts/dev/native-performance.mjs` is a separate opt-in Linux candidate check.
Build `target/release/browsey` with Tauri first, then run:

```sh
BROWSEY_NATIVE_PERFORMANCE_APPROVED=yes node scripts/dev/native-performance.mjs
```

It opens only generated 10k/100k folders in private XDG profiles, verifies exact
candidate PIDs, observes the expected first file row through bounded AT-SPI,
and stops only its own read-only test instances. Six starts per shape distinguish
the first private profile from five repeated-profile starts. Process-tree RSS
is a sum with shared-page double counting, not exclusive physical memory.
Candidate startup/readiness overhead is not folder-only latency or proof of
the first visible thumbnail. No installed-app restart is performed.

The first native observation attempt exposed a WebKit accessibility defect:
the list/grid containers declared ARIA tables without row/cell children, so
AT-SPI reported zero rows and no files. They now expose labelled groups of the
existing native buttons, preserving keyboard/selection behavior without
claiming a table contract. Browser regressions verify both collections and
named file buttons; native readiness must verify the actual generated file,
not just the container or breadcrumb. Accessibility is enabled only in the
disposable candidate process, not through desktop-wide configuration.

The corrected production candidate was built with Tauri's embedded frontend,
SHA-256 `94203f7664b9f3438bfa94358b721b5a548fcce306a2aea0b2b245df233a12aa`.
A follow-up run after the build/browser suites finished observed:

| Generated folder | First private-profile start | Repeated-profile median / max |
| --- | ---: | ---: |
| 10,000 entries | 1,232 ms | 997 / 1,043 ms |
| 100,000 entries | 2,051 ms | 1,978 / 2,080 ms |

There are five repeated-profile samples, not a p95. Parent RSS at row readiness
was approximately 165–175 MiB / 184–185 MiB. Summed parent/descendant RSS was
approximately 459–479 MiB / 539–640 MiB, with shared-page double counting.
The ordinary app remained running and was neither installed nor restarted.

Wall-clock budgets should remain opt-in review targets until repeated native
and storage runs establish stable variance. CI enforces semantic/structural
bounds instead: queue capacity, virtualization, no extra traversal after observed
cancellation, bounded dimensions and correct source/output retention. Proposed
numeric targets and native results are recorded at the verification checkpoint;
they must not be represented as hardware-independent guarantees.

Provisional review targets for this optimized, generated OS-warm workload:

| Path | Review target for median | Basis |
| --- | ---: | --- |
| Listing 10k / 100k | 100 / 750 ms | Measured 54 / 537 ms, with development-host headroom |
| Simple name search 100k | 100 ms | Narrow name-prefilter query only |
| Thumbnail generation at 384 | 200 ms per image | Warm format-specific median, not initialization or remote I/O |
| Mock UI refresh 100k | 200 ms | Measured 103 ms; mock IPC, not native storage |
| Mock UI scroll/zoom | 100 ms | Virtualized rendering; same measured viewport |
| Controlled copy cancellation | 50 ms | Includes a deliberately injected 20 ms read boundary |
| Native candidate start to accessible row, 100k | 3,000 ms | Measured repeated-profile median 1,978 ms on this host |

These are diagnostic review proposals, not agreed user latency requirements,
CI pass/fail thresholds or a claim about slow-device interruption. Compare
individual samples and variance on the same machine/data/profile before making
optimization decisions. Real cold storage and first displayed native thumbnails
still need separate measured budgets.

## Verification Checkpoint

The full working-tree strict maintenance checkpoint passed 652 backend tests
(14 explicitly opt-in tests ignored), 318 frontend tests, 64 browser tests,
Clippy with denied warnings, blocking/advisory Semgrep and 20 strict docs checks.
This checkpoint included the separate cloud acceptance harness additions; it
does not turn ignored real-provider or native tests into automatic acceptance.
The five optimized workload tests and bounded native observation were run
separately. Later transport findings are tracked in the OneDrive checklist.

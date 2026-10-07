# Bounded daily-driver follow-up

Date: 2026-10-07 (Europe/Oslo). Scope: the remaining performance and OneDrive
subtasks in [the engineering TODO](../../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md).
Google Drive and Nextcloud are excluded. No dedicated OneDrive test account is
available; genuine quota exhaustion and deliberately induced provider throttling
remain unverified. All writes use new generated UUID subfolders in the already
approved device/provider test roots. Existing files and historical recovery data
are retained. Status: follow-up in progress.

## Storage opening and thumbnails

Run `b420963f-2f3c-45ac-a7a2-0e8bd0b38819` is native **PASS**: local,
OneDrive and MTP, five samples each, independent byte/SHA-256 checks of all 60
images per provider, and owned AT-SPI. All four process teardown stages and
strict private retention pass (494 entries, 13,959,908 bytes). Duration is
14 min 49 s including setup, 60 independent OneDrive readbacks and teardown;
it is not thumbnail latency.

Candidate: debug `native-test` on `79461066`, dirty only with follow-up changes;
app-source SHA `8cfe1c3faa7ece3e62e8600836f60b7c4663947fc18b1fb3ad716f637d19b241`,
binary SHA `8d3a5d111fc4c4344a7df4011bf9bd224c28da2ce93c9274b687dc1f47ac80cd`,
harness SHA `7bee9a01f0193f083d0de52730cdad404b13d4f5445bbafac2138cead398dbdc`.
GTK 3.24.52, WebKit 2.52.6, Linux 7.2.5-3-omarchy, x86_64, Node v26.10.0.
The timing gestures do not certify a keyboard layout; input layout was not recorded.
No build, competing benchmark or other cloud mutation ran during measurement.

Each sample opens a new 12-file folder of generated 1280×720 RGB PNGs, each
under the ordinary 64 KiB fixture limit. Opening measures actual Enter from the
parent through exact displayed list membership, including driver/native guards.
Thumbnail timing uses the browser monotonic clock from the observer before the
subsequent grid toggle to actual decoded thumbnails. It is **not** Enter-to-first-
thumbnail latency, app startup or backend-only decoder time. Source-byte checks
run after the timings; verification I/O is excluded.

| Provider | Samples | Opening median / max ms | First decoded thumbnail median / max ms | All 12 median / max ms |
| --- | ---: | ---: | ---: | ---: |
| Local | 5 | 97 / 134 | 149 / 190 | 298 / 363 |
| USB (earlier completed subcase) | 5 | 92 / 125 | 133 / 223 | 295 / 364 |
| MTP | 5 | 107 / 164 | 167 / 231 | 367 / 435 |
| OneDrive | 5 | 1643 / 1743 | 4766 / 9235 | 26322 / 33495 |

USB evidence is the fully completed `storage-performance-usb` subcase in run
`509dd1ad-0a32-4c22-815c-95e75587a53f`: five samples and all 60 independent
byte checks **PASS**, as did its earlier local subcase and final teardown/retention.
The aggregate raw run remains **FAIL** after the maintainer requested a pause
and the exact candidate was stopped during later cloud verification. Its cloud
subcase is not acceptance and MTP was not run. The fresh PASS above supersedes
its cloud observations without rewriting the earlier report or repeating USB.
USB candidate source SHA `9179ac91b95bc56ec79e93ede5d472750d6c32d488c59821f0f5a68870928373`,
binary SHA `e27c43b1402244e717fa757a52f9723a73f811390420b021fdaca615e8116371`,
harness SHA `4d03575d42b465156e07fab3bd48cc3223365f21c43967b6b94e3228f568ab37`,
base commit `802d16f2`, dirty follow-up harness. Source changes between these
candidates include the remote-name fix and test-only export guard; this is not
an A/B product-performance comparison.

Local/USB use fsync plus per-file `POSIX_FADV_DONTNEED`, verified with mincore:
all generated source-file pages were nonresident before thumbnail access.
There is no global cache flush, device reset or discard. Directory metadata and
drive firmware caches remain uncontrolled. MTP and OneDrive use fresh names,
private profiles and private thumbnail caches, with device/GVFS/provider caches
uncontrolled. MTP was fast for these small inputs; no controlled slow-MTP condition
or physical cold power-cycle is claimed. Five samples describe median/max;
the raw empirical p95 equals the sample maximum and is not a robust tail estimate.

## Slowest path and budgets

OneDrive is the slowest observed path: roughly 4.8 s median to first thumbnail,
with a 9.2 s maximum. Code inspection confirms cloud thumbnail precheck stats the
remote file, passes that snapshot to shared materialization, downloads source
contents to the private preview cache, then uses the ordinary decoder. The grid
requests at most three thumbnails concurrently. The cache uses a read lock for
materialization and a separate write lock for maintenance; it does not serialize
all downloads behind an exclusive cache lock. RC requests release the daemon
state mutex before HTTP I/O. No duplicate materialization stat or justified
worker-count/cache-format change was identified.

The network/stat/source-download path is a plausible explanation for the local-
versus-cloud gap, **an inference**, not separately measured stage attribution.
The baseline's warn-level logs contain no per-operation timing breakdown.
No JPEG experiment, extra worker or speculative thumbnail change
is included. The transfer inactivity correction below is separate. The existing [performance targets](../../../audits/daily-driver/performance-workloads.md)
remain the starting point: their warm decoder and large-list timings are different
measurement boundaries, not promises for first thumbnails over OneDrive.
The maintainer explicitly agreed the following host/workload review targets
after seeing these measurements, using the existing provisional targets as the
starting point. All four measured medians satisfy them.

| 12-image native workload | Opening median target | First decoded thumbnail median target |
| --- | ---: | ---: |
| Local / USB / MTP | ≤200 ms | ≤200 ms |
| OneDrive | ≤3000 ms | ≤5000 ms |

These are five-sample median review targets for this host, input set and timing
boundary, not individual-response guarantees or end-to-end startup budgets.
The 9235 ms OneDrive outlier remains reported. Broader physical-cold/device and
mixed-large-tree budgets remain separate, and warm engine targets retain their
earlier provisional status. No workstation latency gate was added to CI;
immutable raw timing reports retain the absence of thresholds during execution.

## Prepared cloud copies to Nautilus

Run `31f8c55a-45ef-43f9-a21a-e37d6bd5b830` is native **PASS**: preparation,
cancelled drag, actual receiver copy and preservation, plus AT-SPI. The real
cloud-export modal prepared two generated files, including
`first æ #?%+ spaced.txt`. Trusted XTest input cancelled one drag; the receiver
remained empty. A subsequent copy drag delivered both exact names/bytes to the
owned Nautilus GTK window. Independent byte/SHA-256 checks verify both OneDrive
originals and receiver copies; bounded direct reads verify retained staging.
Cloud originals are preserved. These observations certify private X11 copy,
not shared Wayland receivers, portal behavior or remote move semantics.

Candidate app/source/harness identities match the trash run below. All four
owned-process teardown stages, private desktop closure, outside namespace exit
and strict retention **PASS** (96 entries, 12,823,513 bytes). The only export
exception permits up to two exact generated cloud filenames in this run;
external launch IPC and ordinary-session exceptions stay disabled. No product
defect or change was required.

## Native trash and manual web restore

Run `3bec22d1-a38e-4bf7-8f66-36503b0f8192` is native **PASS**: four parts
(native upload, native trash, manual web restore, preserved originals/restored
bytes), plus AT-SPI. The maintainer explicitly confirmed web restoration of
`browsey-web-restore-3bec22d1-a38e-4bf7-8f66-36503b0f8192.txt` in this conversation.
The independent verifier observed its return to the exact original cloud path,
checked its 72 bytes and SHA-256
`0e1b0413721ea77c6ca4c676fa4a0e33b5abbd09db452b6debfa482228ea5096`,
then verified both local original and cloud file again and explicitly refreshed
the native list. No content upload, automated restore or mutation retry occurred
after trash; the native test only polled the owned source folder during the
finite ten-minute manual window. Restoration was observed at 06:14:35 UTC.

All four owned-process teardown stages, private desktop closure, outside
namespace-supervisor exit and strict retention **PASS** (51 entries, 753,018
bytes). The cloud exception is test-only, exactly one run-specific generated
file; no general cloud-trash, local/system-trash or external launch IPC was
enabled. The shared web session was operated by the maintainer, outside the
isolated native desktop; web UI automation is not claimed.

Candidate: `79461066`, dirty follow-up; app-source SHA
`7933cee29347098cf9aade829fb935fc77c9a90d71472ae61ca48a98cc64b214`,
binary SHA `48a2b062c59b3fa17a2f693acd40d23b09f9506dacbf41e3b87d9ad4b362cf72`,
harness SHA `4128ec7009e6b757d0c3d24f697704cad66127ef39b8a1422c805bd8878ce838`.
Native policy 172/172, native ESLint, the exact-path/mode/input Rust trash guard
regression and 20 strict documentation checks pass. Before the transfer correction, full strict maintenance
also passed: 830 backend tests (19 opt-in tests ignored), 715 frontend tests,
107 mock-IPC browser tests, Rustfmt/Clippy, frontend lint/typecheck/build,
Semgrep blocking zero findings and 20 strict docs checks. Mock tests do not
substitute for native results. No new installed-build, ordinary-use or broader
provider signoff is claimed.

## Server-side destination writer

Run `d5fe969a-d966-4551-acea-e160a1bb4d7e` is native **PASS**, plus AT-SPI.
The real upload pauses at the existing generated five-second finalize checkpoint
after its 26-byte payload is uploaded, before client completion. A separate
bounded rclone process writes a different 26-byte generated file to that exact
owned destination. Backend status independently shows the finalize checkpoint
still held both before and after the competing write. Browsey then acknowledges
the successful copy; independent readback observes the competing writer's
bytes, with original source, actor source and unrelated sentinel preserved.
Task/callback release and all four owned-process teardown stages pass; strict
retention passes (56 entries, 4,943,533 bytes).

This demonstrates the documented stable-destination requirement: another writer
can replace uploaded bytes before client acknowledgement. It does not claim
provider compare-and-swap, transactions, protection during every upload phase,
or atomic trees. The observed completion is truthful for its successful upload;
the operation cannot guarantee subsequent exclusive ownership of the target.
No speculative retry, new consistency engine or product change was added.
Candidate app/source/harness identities match the trash/export runs. Broader
acceptance row A0-7 stays open.

## Mixed-size tree and transfer deadline correction

Run `891af043-3c07-4bf3-ad02-2fe5ac5229d7` is native **FAIL**. Its generated
source has 1028 files, 16 directories (including eight empty directories) and
11,419,904 bytes. Upload failed with the actual UI error `rclone copy timed out
after 300s`; remote digests, download and final tree preservation were not run.
A bounded read-only metadata check of this exact generated destination found
836 files, 16 directories and 11,366,544 bytes retained. The aggregate failure
is not rewritten as acceptance. All four process teardown stages and strict
retention pass (1098 entries, 16,398,396 bytes). The partial destination and
original source remain preserved; no mutation is replayed against this UUID.
Candidate app/source/harness identities match the trash/export runs.

The fixed whole-operation deadline could stop an advancing large cloud copy.
Copy/copyto/move/moveto now use 300 seconds **without measurable progress**;
operation-local increasing byte, completed-file, checked-entry and server-side
work counters reset that clock. Arbitrary messages, unchanged stats, elapsed
job time and errors do not reset it. CLI JSON stats are drained without filling
the bounded diagnostic buffer; malformed/oversized lines remain bounded.
Async RC jobs use only their own group, never aggregate daemon statistics.
Metadata, deletion and individual network request bounds remain. Explicit CLI
timeout overrides retain their total deadline. Cancellation still kills/reaps
the CLI child or requests an RC job stop. A stalled submitted write cannot fall
back to CLI and be repeated; an unconfirmed RC stop reports unknown state.

The installed rclone was checked against two generated local files with a
bounded slow transfer: its actual JSON stats reported increasing bytes and a
completed file, and the destination was byte-identical. This checks telemetry
compatibility, not OneDrive/native acceptance. The rclone interfaces are
[JSON stats logging](https://rclone.org/docs/#use-json-log) and
[operation-specific RC groups](https://rclone.org/rc/#assigning-operations-to-groups-with-_group--value).


After the transfer correction, full strict maintenance passes: **839 backend
(19 opt-in ignored), 715 frontend and 107 mock-IPC browser tests**, Rustfmt,
Clippy deny-warnings, frontend lint/typecheck/build, Semgrep blocking zero
findings and all 20 strict documentation checks. Regressions cover actual
pipe-drained child progress beyond a scaled total deadline, unchanged stats
and errors causing an inactivity stop, explicit total bounds, prompt active
cancellation/reaping, zero-byte/completed work, bounded malformed/oversized
telemetry and retained failure diagnostics. Submitted RC moves as well as
copies cannot replay CLI after an unknown async job state. Native policy
172/172 and native ESLint also pass.


The fresh retest `22456d58-5c57-446b-8b13-9ece00fd2903` is native **PASS**:
all five declared parts and AT-SPI. Both local trees have exactly 1044 entries,
1028 files and 11,419,904 bytes, including all 16 directories and eight empty
leaves. A separate rclone SHA-256/download stream verifies all 1028 actual
OneDrive files before the native download. Source and downloaded sizes/hashes
match independently. All four owned-process teardown stages pass; strict
retention passes (2142 entries, 27,784,261 bytes). The earlier failed UUID is
retained separately.

Candidate: debug `native-test`, base `79461066`, dirty follow-up;
app-source SHA `b926efda7d4e2de295aabe8bcdbb250e23347426586bfd4b113d43b5a0f06c49`,
binary SHA `13c5f64c3910b2e6b642c6aeca8b54d2f1e0ac462cfa249d2f4192f41fb80025`,
harness SHA `388ccb9f4224be2c6071186f4c8fd2a62213f607feedc442044fdf01d8ff145c`.
Built 06:40:58 UTC; run 06:42:29–06:59:28 UTC on the host described above.

| Bounded mixed-size tree workflow | Single observed time |
| --- | ---: |
| Native upload, including navigation/clipboard/refresh | 407,641 ms (6 min 48 s) |
| Native download, including navigation/clipboard/refresh | 428,220 ms (7 min 8 s) |

These are single complete-workflow observations, not isolated transfer-engine
latency, statistically useful medians/tails or agreed budgets. Generated local
source pages were warm. The download follows independent remote content/hash
reads; provider/CDN cache state is uncontrolled. One read-only local destination
name count observed download progress without additional cloud requests.
No competing build/browser suite ran during these native timings. The fresh
UUID and private app profile do not establish physical cold storage.


A separate post-fix read-only preservation check confirms the earlier failed
UUID's complete original local tree: all 1028 files/1044 entries and 11,419,904
bytes still match their generated sizes/SHA-256. Its cloud metadata still has
852 entries, 836 files, 16 directories and 11,366,544 bytes, matching the initial
partial snapshot's counts/total. All names are generated expected names. Two
incomplete retained files, `group-0/file-062.bin` and `group-0/file-063.bin`, are
0 bytes instead of their original 64/1024 bytes. These are failed-operation
partial outputs, not accepted complete copies. The separate full-content
hashsum stream returned `FIXTURE_IO`; no complete hash acceptance is claimed
for the old partial destination. Neither these objects nor the original failed
report was repaired, deleted or replayed. Private supporting receipts/metadata
use a separate generated profile; the sealed native run remains untouched.
This standalone verification is not another native acceptance run. The fresh
native PASS above independently verifies every file in its own complete copy.

## Remaining scope

The bounded native cases above are complete. Real quota/rate-limit scenarios
remain unverified without a dedicated account; broader cold-device conditions,
repeated mixed-tree measurements and agreed mixed-tree performance budgets stay
separate. Native export, the destination writer and manual web restore completed
above. Google Drive and Nextcloud remain outside this requested scope.

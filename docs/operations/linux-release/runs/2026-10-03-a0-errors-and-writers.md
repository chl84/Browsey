# A0 Error, Cancellation and Concurrent-Writer Follow-Up

Date: 2026-10-03.
Tester: maintainer-directed coding agent; not independent human signoff.
Repository starting baseline: `30a9b9dded29f44a934df289e766634ef97e8b40`.
Installed app: Browsey 1.0.3, production binary from `bf82750`, as in the
[previous native run](2026-10-03-a0-native-operations.md).
Installed SHA-256 rechecked before and after candidate building:
`740eb5ab0df40f26191069d148c4819920de33467185ed2dfe853f3d9ed8a12b`.
Environment: Omarchy 4.0.4, Hyprland / Wayland, Linux 7.2.5-3-omarchy, x86_64;
fixtures and private XDG homes on the home Btrfs filesystem.

The final two native checks used a separate optimized candidate matching
`9b1e2ea960758fc7f866df8ac5675da665f0cc9a`, including VD-1 and VD-2. Its SHA-256 was
`cb7ab6b6a600e351c90ae90ab711bbbffb8751a4873ed39d478abdd4437b5d2f`.
It was built with `tauri build --no-bundle -- --locked` and run directly, not
installed over the ordinary app. No version bump, package publication or
personal app restart was performed.

Expected behavior: [core operations matrix](../../core-operations/matrix.md).
These are fifteen scoped subchecks, not fifteen completed acceptance-index rows.

## Isolation and Method

All inputs, destinations, ZIPs and test configuration were generated in a private
`mktemp -d` state directory, `browsey-a0-faults.oy6x7r`. Each native instance opened
a fixture folder with separate XDG data/config/cache/state. The helper validated
the exact executable, PID and isolated profile before process-targeted compositor
shortcuts or AT-SPI actions. Candidate execution additionally used an explicit
executable whitelist. Filesystem assertions independently checked outcomes.

No personal files, USB device, phone, network mount or cloud account were changed.
Native clipboard exports contained only generated fixture paths. Permission
denial used an owned fixture directory with mode 0500, restored to 0700 afterward.
All owned Browsey test processes were stopped after completed operations; no
Browsey PID remained. The 55 MiB fixture root was retained for inspection.

Tests F-8, F-9, F-12 and F-13 used a fixture-scoped `LD_PRELOAD` shim to delay
successful writes to `slow.bin` by 150 ms. It did not inject failures or change
bytes and was not installed or loaded into personal apps. This made cancellation
and second-process writes reproducible, but is **not** a real slow-storage test.
Other installed scenarios and the candidate checks used unmodified runtime I/O.

Initial path-field navigation attempts did not reliably commit their values and
were discarded. Accepted operations instead launched folder-specific instances
and confirmed accessible breadcrumbs. An initial background context menu was
also discarded; batch extraction used a focused file-list caret/selection and
the actual selection menu. Delivered input alone was never counted as a pass.

## Completed Native Subchecks

- [x] **F-1 / copy conflict Cancel:** Cancelled the native conflict dialog. Source,
  existing destination and unrelated peer retained their distinct original
  bytes; no extra output was created.
- [x] **F-2 / copy Auto-rename:** Created `alpha-1.txt` with exact source bytes while
  preserving source, original destination and unrelated peer. Undo removed only
  the new file.
- [x] **F-3 / copy Overwrite and history:** Destination became byte-identical to
  source. Undo restored its distinct original bytes; redo restored copied bytes.
  Source and unrelated peer survived every transition.
- [x] **F-4 / move conflict Cancel:** Cut/paste cancellation preserved both original
  files and their distinct contents.
- [x] **F-5 / move Auto-rename and history:** Source moved to `alpha-1.txt`, leaving
  the occupied `alpha.txt` untouched. Undo restored source and removed only the
  renamed output.
- [x] **F-6 / move Overwrite and history:** Source moved into the occupied target.
  Undo restored both originals; redo moved source again with correct bytes.
  These are same-filesystem moves, not cross-device fallback acceptance.
- [x] **F-7 / permission denial:** Both copy and cut/paste into the mode-0500
  directory reported an open-target `Permission denied (os error 13)` error.
  Original source bytes survived and the destination stayed empty. No automatic
  destructive retry occurred.
- [x] **F-8 / cancel mid-copy and listing:** Observed writes during a 16 MiB copy,
  then clicked Cancel task. Native error described retained uncertain output
  and advised inspection before retrying. Source SHA-256 remained
  `080acf35a507ac9849cfcba47dc2ad83e01b75663a516279c8b9d243b719643e`.
  The retained 2 MiB partial output stayed stable after cancellation. Selecting
  and copying the refreshed native listing exported that output's URI; the
  completed error state no longer exposed Cancel task.
- [x] **F-9 / cancel mid-ZIP extraction:** Observed 8 MiB of output from the 16 MiB
  fixture before cancellation. Native status said “Extraction cancelled”; the
  extractor rolled back its owned output tree and retained the source ZIP.
- [x] **F-10 / extraction permission denial:** ZIP extraction under a mode-0500
  parent reported inability to create the destination with `Permission denied
  (os error 13)`. The archive remained and no phantom output folder was created.
- [x] **F-11 / partial extraction and listing:** Batch-selected one valid and one
  deliberately broken ZIP. Installed status counted one success and one failure;
  the valid output bytes matched input, the bad archive remained unchanged and
  no bad output was created. Native listing export contained both archives and
  the new good folder. Count-only feedback reproduced VD-2, corrected below;
  this check accepts filesystem/listing behavior, not the old message's adequacy.
- [x] **F-12 / real destination writer:** A separate Node process wrote
  `foreign-target-edit` into the already-written output prefix after 512 KiB had
  been copied. The installed engine refused completion with “Copied content does
  not match the written stream”. Source SHA-256 stayed unchanged; foreign output
  bytes were retained. It did not delete the uncertain result or retry.
- [x] **F-13 / real source writer:** A separate process wrote `foreign-source-edit`
  into the source prefix after 512 KiB of output. Browsey refused completion with
  “Source changed during copy” / “File version or length changed during copy”.
  The edited 16 MiB source and uncertain destination remained. The source hash
  from F-8/F-12 is not claimed unchanged after this deliberate edit.
- [x] **F-14 / corrected native candidate partial result:** A fresh good/bad ZIP
  batch reported “Extracted 1 archive, 1 failed” followed by
  `bad.zip: Failed to read zip: invalid Zip archive: Could not find EOCD`.
  Good output matched input; both archives remained; native listing export
  contained exactly those archives and the good output folder.
- [x] **F-15 / corrected native candidate RAR Settings:** The optimized app showed
  the shared compressed/password extraction note and unsupported RAR-creation
  boundary. Typing `passwords` into Filter settings retained that note. This
  complements the earlier mock-browser and ten backend RAR checks, not a new
  physical encrypted-archive acceptance test.

The conflict dialog offers Cancel, Auto-rename and Overwrite. **Skip is N/A** for
this dialog because it is not a supported action; no Skip feature was invented
or marked tested. Non-archive skipping in extraction remains covered by existing
unit tests, not a new native scenario here.

## Confirmed Finding and Small Fix

VD-2: the installed mixed archive batch gave counts but no failed filename/reason.
Four new unit regressions failed against the baseline: missing batch details,
refresh failure hiding partial outcomes, refresh failure misreporting a successful
single extraction as failed, and failure not refreshing retained output.

The existing file-operations controller now reports up to three failed archive
names/reasons with an explicit additional-failure count. Error/detail notices use
the existing toast with a longer reading interval. Listing refresh is best effort
after completion, error or cancellation; its failure adds F5 guidance without
changing the true extraction result or repeating extraction. Two further
regressions cover detail truncation and mid-batch cancellation plus refresh error.
All six new cases and all 48 tests in the file-operations suite pass. Actual
listing-failure injection was unit-level; F-14 independently verifies the native
partial-result message and successful refresh, not a native induced-refresh fault.

The [concurrent-writer review](../../../audits/daily-driver/concurrent-writer-boundary.md)
documents the existing guards and stable-tree support boundary. No backend
algorithm was replaced. Real writer observations and deterministic final-gate
tests do not close after-readback or final check-to-unlink windows.

## Remaining Acceptance

A0-2, A0-3, A0-5, A0-7 and A0-8 remain **PARTIAL**. The
[acceptance index](../daily-driver-validation-checklist.md) still has 27 open
top-level rows and the two previous scoped passes. The engineering review and
VD-1/VD-2 are checked independently; they are not full daily-driver signoff.

Still untested here: real full/disconnected destinations, disappearing sources,
device removal, power loss, native cross-device fallback, arbitrary final-check
races, other distributions, broader filename/symlink matrices, Wastebasket
restore/purge, all action histories and the full 50-action/session limit. Permission
fixtures are not read-only-media acceptance. No physical device was reformatted.
No ordinary-use stabilization period or release signoff is claimed.

## Automated Verification

The optimized production build passed. Separate final checks passed all 616
backend tests (five opt-in tests ignored), all 306 frontend tests in 40 files,
warnings-denied Clippy, Rust formatting, frontend lint and typecheck with no
errors/warnings. Native interaction supplemented these checks; mocked IPC is
not proof of hardware or desktop integration.

`CARGO_NET_OFFLINE=true bash scripts/maintenance/test-both.sh --strict-docs`
(Cargo 1.98 on PATH) also passed: 28 release-helper
tests, backend resource/vendor/dependency/error-policy gates, formatting/check/
Clippy, 616 backend tests, frontend lint/typecheck, 306 unit tests, 60 existing
mock-IPC browser smoke tests, production frontend build and all 20 strict docs
consistency checks. The existing automated browser suite supplements the T3
shared-preview RAR check; it is not native device acceptance. Five opt-in backend
tests stayed ignored. Semgrep is unavailable and skipped, not passed.

After the final documentation edits, all 20 strict consistency checks and all
51 local links across the affected run/audit/checklist/TODO/README documents
passed. Checkbox assertions confirmed fifteen native subchecks and the unchanged
two scoped passes / 27 open acceptance rows. `git diff --check` passed. These
documentation checks do not imply a new installation or release signoff.

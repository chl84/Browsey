# A0 Installed Native Operations Follow-Up

Date: 2026-10-03.
Tester: maintainer-directed coding agent; not independent human signoff.
Repository baseline: `fa3807737701285a8c8c39e0a0d458d3129e6418`.
Installed candidate: Browsey 1.0.3, the production build from `bf82750` installed
in the [earlier Omarchy run](2026-10-03-a0-omarchy.md). Subsequent repository
changes before this run were documentation-only; no rebuild was necessary.
Installed binary SHA-256, independently rechecked:
`740eb5ab0df40f26191069d148c4819920de33467185ed2dfe853f3d9ed8a12b`.
Environment: Omarchy 4.0.4, Hyprland / Wayland, Linux 7.2.5-3-omarchy, x86_64.

Scope: selected installed local move, rename, trash, permanent-delete and ZIP
extraction flows. Expected behavior remains the
[core operations matrix](../../core-operations/matrix.md). These checks contribute
to A0-2, A0-3 and A0-5 in the
[acceptance index](../daily-driver-validation-checklist.md); none is full matrix-row,
whole-A0, hardware, cross-distribution or release signoff.

## Isolation and Method

A private `mktemp -d` directory in user state held all generated sources,
destinations, archives and separate XDG data/config/cache/state homes. Unlike the
earlier tmpfs fixtures, this root and its private XDG trash were on the same home
Btrfs filesystem. Six installed launcher instances opened only fixture folders.
`NO_AT_BRIDGE=0` enabled process-specific AT-SPI inspection without changing user
desktop configuration. No personal file, physical device, network or cloud data
was mutated. Native cut/copy tests exported only generated fixture paths.

The targeted input helper checked the exact installed executable, test PID and
isolated XDG profile before every command. It resolved that PID's compositor
window and used `hl.dsp.send_shortcut` for window-targeted shortcuts. AT-SPI focused
the intended list/input and invoked uniquely identified modal buttons. Assertions
independently read the real filesystem after operations; delivered input alone
was not considered a pass. All six test processes were stopped after completed
operations, and their exits were independently verified. Fixtures were retained
for inspection; no personal Browsey process was stopped.

Early global-input attempts were discarded when focus moved elsewhere. A file
rename left an intermediate `renamed-æøå.txt.txt` during those ambiguous attempts;
that name is not evidence of a Browsey naming defect. The accepted rename below
used an explicitly focused field whose value was verified before submission.
An off-workspace folder Escape attempt left transitional accessible nodes, so
keyboard Escape/modal teardown is **not** accepted from that attempt. Successful
folder rename and filesystem undo/redo were independently verified afterward.
This run does not certify focus restoration, Tab trapping or screen-reader use.

## Completed Subchecks

These ten boxes track distinct, narrowly scoped checks, not ten completed rows
of the broader acceptance index.

- [x] **N-1 / CO-LCM-003:** Native cut/paste moved `alpha.txt` to an empty destination.
  Source disappeared only in the completed result; destination had the exact
  expected bytes, `Native move fixture: preserve these bytes.` followed by LF.
- [x] **N-2 / CO-LCM-004:** The same selected batch moved a nested directory with
  `Blåbær.txt`. Original tree disappeared; destination child retained its exact
  expected content. This is same-filesystem move, not cross-device move acceptance.
- [x] **N-3 / A0-5:** Move Ctrl+Z restored both source entries and emptied the target;
  Ctrl+Y moved them again. Both file contents were checked after each transition.
- [x] **N-4 / CO-LRN-001, A0-5:** Native file rename produced `accepted.txt` with
  unchanged content. Undo restored its immediate previous name and bytes; redo
  restored `accepted.txt`. The earlier intermediate name is described above.
- [x] **N-5 / CO-LRN-002, A0-5:** Native folder rename changed `nested` to
  `folder-renamed`; the Unicode child remained byte-identical and unrelated
  `alpha.txt` was preserved. Undo and redo restored the respective tree names
  with unchanged child bytes.
- [x] **N-6 / CO-LRN-003, A0-3:** A rename to seeded `existing.txt` stayed in the
  modal with “A file or directory with that name already exists”. Both original
  names and distinct contents survived. Clicking Cancel closed the modal without
  changing either file. No silent overwrite or automatic destructive retry occurred.
- [x] **N-7 / CO-LTD-001, A0-5:** Delete moved the generated trash fixture into the
  private XDG trash. Its data bytes and decoded `.trashinfo` original path matched.
  Undo restored its source bytes; redo returned it to private trash; another undo
  restored it and left no data file in that private trash. This is history undo,
  **not** acceptance of the Wastebasket Restore or Purge controls.
- [x] **N-8 / CO-LTD-005, A0-3, A0-5:** Shift+Delete opened the permanent-deletion
  confirmation. Clicking Cancel retained the exact original bytes. Reopening and
  clicking Delete removed the fixture; undo restored its bytes, redo removed it,
  and another undo restored it. No persistent recovery guarantee is implied.
- [x] **N-9 / CO-EXT-001, A0-5:** Opening the generated ZIP extracted `readme.txt`
  and `nested/æøå.txt` into `fixture`. Both files matched the archive input bytes;
  native status reported the destination. Undo removed only the output tree;
  redo restored both files. The original ZIP stayed intact.
- [x] **N-10 / CO-EXT-002:** Repeated extraction selected `fixture-1`, then
  `fixture-2`, consistent with the extractor's unique-destination policy. Before
  the last extraction, the original output's `readme.txt` was deliberately edited
  and an unrelated `keep.txt` added. Both distinct contents survived; both newly
  extracted files matched archive inputs. Native status named `fixture-2`.
  The ZIP SHA-256 remained
  `e45bda1b6e5af290f31d08bec13e17be0d2d35fdeea92b7ebbad69dc0bfdb5da`.

## Results and Remaining Scope

A0-2, A0-3 and A0-5 remain **PARTIAL**. There are still 27 open top-level acceptance
rows; A0-4 and A0-6 retain only their previously recorded scoped passes. Native
checks above supplement, rather than replace, the earlier backend and frontend
test evidence. Those suites were not rerun in this documentation-only follow-up.

Still outstanding: move/copy overwrite policies and cancellation during work;
directory trash and Wastebasket restore/purge; extraction mid-run cancellation,
permission failures and mixed-result batches; full/disconnected destinations,
disappearing sources, symlinks, long/non-UTF-8 names; installed partial-result
refresh and active-writer races. Native create/compress/batch-rename histories,
overwrite boundaries and the full 50-action/session-only UX limits also remain
outside this run. Do not equate working Undo with persistent history or guaranteed
recovery; see [undo scope](../undo-scope.md).

The isolated log contained an initial missing-home-trash warning before the
private trash was created. The later trash flow succeeded as independently
verified above. No crash was observed, but this is not a crash-diagnosis or broad
logging acceptance run. No new confirmed application defect was established;
the previously recorded VD-1 Settings wording remains unfixed.

Next: installed conflict/cancel/error and partial-result refresh scenarios with
controlled fixtures, followed by concurrent-writer acceptance. Device faults
require separately approved disposable hardware. Application code, installation,
desktop configuration and release artifacts were not changed in this run.

## Documentation Verification

`bash scripts/maintenance/check-docs-consistency.sh --strict` passed all 20 checks.
All 16 local links in this record and the updated acceptance index resolved.
Checkbox counts were independently checked: ten completed subchecks here, two
previous scoped passes and 27 open rows in the acceptance index. `git diff --check`
passed. This is documentation validation, not a rerun of application suites or CI.

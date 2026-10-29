# Defect Review: Trash, Open With, Activity and Navigation

Date: 2026-10-23.
Reviewed source baseline: `899a747d` (`main` after PR #23).
Follow-up: see [validity validation](#validity-validation-2026-10-23) for the
independent reproductions, corrected scope and recommended priorities.
See [implemented fixes](#implemented-fixes-2026-10-23) for the subsequent changes
and regression evidence.
Scope: a targeted search for serious defects in file mutation, launching,
operation progress and navigation flows. This is a review snapshot; nothing
described below has been fixed at this baseline.

## Method

The review read the highest-risk code paths: local paste/move, permanent
deletion, trash, rename, compression, extraction path handling, "Open with",
console launching, cloud transfers, keyboard gating and the navigation and
progress controllers changed in PR #23.

Findings marked **verified** were reproduced with temporary unit tests against
the baseline. Those tests were removed again and the working tree is unchanged.
Other findings come from reading the code, with the triggering sequence traced
through the listed lines. They have not been reproduced in the running app.

## Summary

| ID | Severity | Area | Defect | Evidence |
| --- | --- | --- | --- | --- |
| D1 | High | Trash | Move to trash first copies the entire item into the undo area; it fails for symlinks, unreadable and special files | Verified |
| D2 | Medium | Trash | A failed or cancelled batch leaves earlier items in the trash under staging names, and restores them from backup copies too | Verified |
| D3 | Medium | Open with | Field codes are expanded again inside the substituted file path, so another file can be opened | Verified |
| D4 | Medium | Progress | Concurrent operations share one activity slot; a newer one detaches the older one's progress and Cancel | Code reading |
| D5 | Medium | Navigation | Recent, Starred, Network and Wastebasket loads can override newer navigation | Code reading |
| D6 | Low | Open with | `Terminal=true` applications are launched without a terminal | Code reading |
| D7 | Low | Console | Successfully launched terminals are never waited for and remain as zombie processes | Code reading |
| D8 | Low | Paste | Conflict names are awkward for dotfiles and multi-part extensions | Code reading |

Unconfirmed concerns and design risks are listed after the findings.

## D1: Move to trash pre-copies the entire item

Severity: High.
Locations:
- [trash/move_ops.rs](../../../src/commands/fs/trash/move_ops.rs) lines
  99-127 (`prepare_trash_move`, batch path) and 322-386
  (`move_single_to_trash_with_backend`).
- [undo/path_ops.rs](../../../src/undo/path_ops.rs) lines 16-50 and 175-219
  (the copy routine used for the backup).
- [undo/backup.rs](../../../src/undo/backup.rs) lines 383-440 (backup
  location).

### Behavior

Before calling the system trash, both trash paths run
`undo_copy_entry(&src, &backup)` (move_ops.rs lines 116 and 342). This copies
the whole file or folder tree into
`<data_dir>/browsey/undo-sessions/...` (normally
`~/.local/share/browsey/undo-sessions`). The copy is removed only after the new
trash item has been found again (lines 217 and 375). The copy routine:

- rejects any symlink in the tree (`ensure_existing_path_nonsymlink`,
  path_ops.rs line 206);
- opens every file as a regular file only
  ([fs_utils/mod.rs](../../../src/fs_utils/mod.rs) lines 54-71: "Only regular
  files can be read"), so FIFOs, sockets and device nodes fail;
- needs read permission on every file, although trashing itself only needs
  write permission on the parent directory;
- streams every byte through a hashing writer and calls `sync_all` per file
  (path_ops.rs line 46).

### Impact

- **Common folders cannot be trashed.** Any folder containing a symlink fails,
  for example `node_modules/.bin`, Python virtual environments and many source
  trees. So does any folder containing a file the user cannot read. The user
  sees "Move to trash failed: Symlinks are not allowed in path: …"
  ([createSelectionActions.ts](../../../frontend/src/features/explorer/file-ops/createSelectionActions.ts)
  line 111). The same items *can* be permanently deleted on the same
  filesystem, because that path renames rather than copies.
- **Large items are slow.** Trashing a 50 GB folder first writes and syncs
  50 GB. Trashing is normally a near-instant rename.
- **Temporary double disk usage.** The data directory needs free space equal to
  the item size. On a nearly full disk, which is a typical reason for
  deleting, the copy can fill the disk before failing. Other applications can
  then fail as well.
- **Items on other filesystems are copied across devices.** Trashing from a USB
  drive or second partition copies the data onto the home filesystem first.
- **Cancel does not interrupt the copy.** The batch loop checks cancellation
  only between items (move_ops.rs line 164). `prepare_trash_move` and the
  single-item path contain no cancellation check, and progress is reported
  per item only.
- **Backups can be retained without expiry.** If the trash item is not found
  again, the operation records `Action::Delete` and keeps the full copy (lines
  223 and 380). The user guide notes that stored backups have no automatic
  expiry ([usage.md](../../usage.md) line 241).

### Reproduction

Temporary test calling `move_single_to_trash_with_backend` with the
fake trash backend from `src/commands/fs/trash/tests.rs`:

| Input | Result | Trash backend calls |
| --- | --- | --- |
| Folder containing `link -> a.txt` | `Err("Symlinks are not allowed in path: …/project/link")` | 0 |
| Folder containing a mode `000` file | `Err("Failed to open source file …/secret.txt: Permission denied (os error 13)")` | 0 |

Both failures happen before the trash backend is called, so the real system
trash behaves the same way.

Manual reproduction: create `~/t/project/a.txt`, run
`ln -s a.txt ~/t/project/link`, select `project` in Browsey and press Delete.

### Related path: permanent deletion across filesystems

Permanent deletion moves the item into the undo area with `move_with_fallback`.
Across filesystems this falls back to `move_by_copy_delete_noreplace`, which
uses the same `copy_entry_recorded` routine (path_ops.rs lines 303-334).
Permanently deleting a folder with symlinks from a USB drive is therefore
expected to fail in the same way, and successful deletions copy the data onto
the home filesystem. This follows from the code; it was not reproduced
separately.

### Recommended fix

Do not copy content before trashing. The staged rename already gives the item
a unique name (`browsey-trash-stage-…`,
[staging.rs](../../../src/commands/fs/trash/staging.rs) line 56), so the new
trash entry can be identified by that staged path. If it still cannot be
identified, report that the item was moved to the trash without Browsey undo,
instead of keeping a hidden full copy. Add regression tests for symlinks,
unreadable children, FIFOs and a large-file timing or space guard.

## D2: Batch rollback leaves trashed duplicates

Severity: Medium.
Location: [trash/move_ops.rs](../../../src/commands/fs/trash/move_ops.rs)
lines 87-96 (`rollback_prepared_trash`) and the calls on lines 165, 182 and 191.

### Behavior

When a later item in a batch fails or the batch is cancelled, earlier items
have already been moved to the system trash under their staging names.
`rollback_prepared_trash` runs `Action::Delete { path, backup }` backwards. That
restores each original path from its **backup copy** (D1) but leaves the
already-trashed item in the system trash. The trash info has not yet been
rewritten to the original path, because that happens only after the loop
(line 210).

### Impact

- The Wastebasket contains entries named
  `browsey-trash-stage-<pid>-<nanos>-<n>`. Restoring one returns it under that
  name.
- The data now exists twice: restored in place and in the trash.
- The trigger is common in combination with D1. For example, select a file
  and a project folder that contains a symlink, then press Delete.

### Reproduction

Temporary test with the fake backend: batch `[report.txt, project/]`, where
`project/` contains a symlink.

```text
RESULT: Err("Symlinks are not allowed in path: …/project/link")
first restored: true
trash delete calls: [".../browsey-trash-stage-243593-1792711602302479125-0"]
trash items after rollback: [("browsey-trash-stage-243593-1792711602302479125-0", "...")]
rewrite calls: 0
```

The existing test `move_to_trash_many_rolls_back_previous_on_later_failure`
checks only that the sources exist again. It does not check the trash contents.

### Recommended fix

Roll back earlier items by restoring the trash entry they were moved to, using
the trash backend's restore. Do not recreate them from a copy. Fixing D1 removes
the copy, so the rollback must be rewritten in any case. Extend the existing
test to assert that the trash is empty after rollback.

## D3: "Open with" re-expands field codes inside file paths

Severity: Medium. A different, existing file can be opened and edited.
Location: [open_with/linux.rs](../../../src/commands/open_with/linux.rs)
lines 454-519 (`command_from_exec`).

### Behavior

Each `Exec` token is rewritten by a chain of `replace` calls. `%f`, `%F`, `%u`
and `%U` are replaced first. The `%d`, `%D`, `%n`, `%N`, `%k`, `%c`, `%i`,
`%m` and `%%` replacements then run over the resulting token, which by then
contains the file path. A `%` sequence in the file or folder name is therefore
treated as a field code. The Desktop Entry specification expands field codes
once, and `%%` only in the original `Exec` value.

### Reproduction

Temporary test with `Exec=viewer %f`:

| Target | Argument passed to the application |
| --- | --- |
| `/home/u/report%i.pdf` | `/home/u/report.pdf` (another file) |
| `/home/u/50%discount.pdf` | `/home/u/50/home/uiscount.pdf` |
| `/home/u/a%%b.txt` | `/home/u/a%b.txt` |
| `/home/u/x%nz.txt` | `/home/u/xx%nz.txtz.txt` |

### Recommended fix

Expand field codes in one left-to-right pass over the original token, without
rescanning substituted text. A better option may be to launch through
`gio::DesktopAppInfo` (`launch` with a `gio::File`). The module already uses GIO
to resolve applications, and GIO also handles `Terminal=true` (D6). Add the four
inputs above as regression tests.

## D4: Concurrent operations lose progress and Cancel

Severity: Medium. No data loss, but a long transfer can become impossible to
cancel from the UI.
Locations:
- [createActivity.ts](../../../frontend/src/features/explorer/hooks/createActivity.ts)
  lines 30 and 71-122 (single owner; `start` takes ownership and runs an
  unscoped `cleanup()`).
- Unscoped cleanup calls (no event name): see the table below.

### Behavior

The activity controller holds one owner, one progress listener and one
generation counter. Each `start()` makes the new operation the owner and
increments the generation. It also unregisters the previous operation's
listener. The earlier operation's `report` closure then ignores all further
progress, and its Cancel button disappears.

The backend permits concurrent work: `backup_operation()` is a shared read lock
([undo/maintenance.rs](../../../src/undo/maintenance.rs) lines 9-13). The
frontend blocks only a second paste (`pasteBusy`,
[useExplorerFileOps.ts](../../../frontend/src/features/explorer/file-ops/useExplorerFileOps.ts)
lines 570-571). It does not block deletion, rename, compression or extraction
during a paste.

PR #23 scoped `cleanup`, `clearNow` and `hideSoon` for rename, new folder,
cloud loading and cloud opening. These callers remain unscoped and act on
whichever operation currently owns the UI:

| Caller | Lines |
| --- | --- |
| `createSelectionActions.ts` (trash/delete) | 96, 115-116 |
| `useExplorerFileOps.ts` paste failure and release | 173-174, 203 |
| `useExplorerFileOps.ts` paste completion | 381, 520, 532, 559 |
| `useExplorerFileOps.ts` extraction `finally` | 865-866 |
| `compressModal.ts` | 94-95 |
| `deleteConfirmModal.ts` | 87-90 |
| `CloudExportModal.svelte` | 31 |
| `SettingsModal.svelte`, `CloudWorkingCopies.svelte`, `recoveryBackups.ts` | 193-201; 123, 145; 84-85, 100 |
| `useExplorerPageLifecycle.ts` mount started/done | 38, 58 |

### Scenarios

1. Start pasting a large folder. While it runs, press Delete on another file.
   The trash operation takes over, finishes and hides the activity. The paste
   continues with no progress and no Cancel. Another paste is then refused with
   "Finish or cancel the current transfer first", but nothing can be cancelled.
2. Start an extraction, then a paste. When the extraction ends, its `finally`
   runs `clearNow()` and `cleanup()`. That removes the paste's progress and
   unregisters its listener.
3. A user-initiated mount (`mounting-started`) clears the current activity and
   replaces it with "Connecting…". This one recovers on the transfer's next
   progress event, because the listener survives.

### Recommended fix

Track activities per event name (a map, or a stack that restores the previous
entry), and scope every `cleanup`, `clearNow`, `hideSoon` and `hasHideTimer`
call. Alternatively, refuse a new mutation while another cancellable operation
is active, with a clear message. Add controller tests with two overlapping
operations that finish in both orders.

## D5: Special-view loads can override newer navigation

Severity: Medium. Wrong view and lost forward history; no data loss.
Location: [explorer/state.ts](../../../frontend/src/features/explorer/state.ts)
lines 287-385 (`loadRecent`, `loadStarred`, `loadNetwork`, `loadTrash`).

### Behavior

Directory loads own a request token. `loadDetailed` discards its result when
`directoryRequest` or `searchRunId` changed (lines 220 and 240). The four
special-view loaders have no such token. They call `invalidateSearchRun()` when
they start, which cancels older directory loads, but after their own `await`
they set `current`, `entries` and history unconditionally (for example lines
351, 357 and 375). Their `finally` also clears `loading` even when a newer
directory load owns it.

The navigation layer's generation check
([useExplorerNavigation.ts](../../../frontend/src/features/explorer/navigation/useExplorerNavigation.ts))
skips only selection restore and scrolling. It cannot stop the raw loader from
committing its state.

### Sequence

1. Click **Network**. `handlePlace` forces a network refresh, which can take
   several seconds of discovery.
2. Before it finishes, navigate with a sidebar place (`handlePlace`, line 403
   calls `loadDir` directly), Back/Forward (`navigateTo`, state.ts line 389) or
   by opening a folder (`open`, state.ts line 466).
3. The folder loads and is shown.
4. Network finishes. The view switches to Network, and `pushHistory` truncates
   the forward history and appends Network.

The same applies to Wastebasket, Recent and Starred, and between two special
views. Bookmarks, typed paths and partitions are not affected, because they are
queued while `loading` is true (`loadDirIfIdle`).

### Recommended fix

Give all five loaders one shared request token. Before committing state and in
`finally`, check that the request is still current, matching `loadDetailed`.
Add tests in which a slow special-view load resolves after a newer directory
load.

## D6: Terminal applications launch without a terminal

Severity: Low.
Location: [open_with/linux.rs](../../../src/commands/open_with/linux.rs) lines
99-114, with `Terminal=` parsed on line 337.

`launch_desktop_entry` ignores `entry.terminal` and starts the program with
standard input and output set to `/dev/null`. The Open with dialog lists these
applications (for example Neovim and htop), but choosing one shows nothing; the
process exits or runs on invisibly. Fix: launch through GIO (see D3), wrap the
command in one of the terminal candidates from
[console.rs](../../../src/commands/console.rs), or hide `Terminal=true`
entries.

## D7: Console launches leave zombie processes

Severity: Low.
Location: [console.rs](../../../src/commands/console.rs) lines 131-134.

`Command::spawn()` succeeds and the `Child` is dropped without `wait()`. Many
terminal launchers (for example `gnome-terminal`) hand off to a server and
exit quickly, so each "Open in console" can leave a zombie until Browsey exits.
"Open with" already avoids this through `spawn_detached`, which waits on a
background thread
([open_with/mod.rs](../../../src/commands/open_with/mod.rs) lines 166-180); the
console launcher should use the same approach.

## D8: Conflict names for dotfiles and multi-part extensions

Severity: Low (cosmetic).
Location: [clipboard/mod.rs](../../../src/clipboard/mod.rs) lines 173-193
(`rename_candidate`).

The name is split at the last `.`. A conflicting `.bashrc` becomes `-1.bashrc`,
and `archive.tar.gz` becomes `archive.tar-1.gz`. `fs_utils::unique_path` treats
the same names differently (`.bashrc-1`), so the naming is also inconsistent
between operations.

## Unconfirmed Concerns

These need targeted confirmation before they are treated as defects:

- **Silent no-op for Google Drive file moves.** File moves use
  `backend moveid` with `--immutable --ignore-existing` and
  `--error-on-no-transfer=false`
  ([rclone/drive.rs](../../../src/commands/cloud/providers/rclone/drive.rs)
  lines 613-628). If the destination name appears after the conflict preview,
  rclone is expected to skip the move and leave the source in place, and
  Browsey would report success. This expectation about rclone has not been
  tested.
- **Case-only renames on FAT/exFAT.** Case-only renames get special handling
  only on MTP ([undo/path_ops.rs](../../../src/undo/path_ops.rs) line 236). On a
  case-insensitive filesystem such as vfat or exfat, a no-replace rename from
  `a.txt` to `A.txt` may be refused as "already exists". This needs a test on a
  FAT USB stick.
- **Chained batch renames.** `rename_entries_impl`
  ([rename/ops.rs](../../../src/commands/rename/ops.rs)) applies pairs in
  request order, so `a→b, b→c` fails and rolls back because `b` still exists.
  This is safe but restrictive. Whether the advanced-rename preview prevents
  such batches was not checked.

## Design Risks (Documented Behavior)

- Permanent deletion moves items into the undo area, and stored backups have no
  automatic expiry ([usage.md](../../usage.md) lines 241-248). Permanently
  deleting therefore does not free disk space until backups are cleared in
  Settings > Stored data. Across filesystems, it also copies the data onto the
  home filesystem (see D1). Consider showing the retained size, or offering a
  real delete for large items.

## Reviewed Without Findings

- Local paste/move, including the overwrite backups, the merge and
  copy-verify-delete move fallbacks, and the "into itself" and "parent of
  source" guards ([clipboard/ops.rs](../../../src/clipboard/ops.rs),
  [clipboard/mod.rs](../../../src/clipboard/mod.rs)).
- Extraction path safety: traversal and absolute components are rejected,
  symlink entries are skipped, existing files are not overwritten, and
  directory modes are applied only to created directories
  ([decompress/util/path_ops.rs](../../../src/commands/decompress/util/path_ops.rs),
  [zip_format.rs](../../../src/commands/decompress/zip_format.rs)).
- Compression output naming and exclusive creation
  ([compress/pathing.rs](../../../src/commands/compress/pathing.rs)).
- Single and batch rename target validation and rollback.
- Permission changes (non-recursive, symlinks refused).
- Keyboard gating: shortcuts are suppressed while blocking modals are open and
  in editable fields. Deletion uses only visible, filtered selected entries.
- The PR #23 Rust changes: statvfs-based free-space checks, the watcher root
  list and the backup lock release.

## Verification Log

All commands were run on 2026-10-23 at `899a747d` with temporary tests appended
to the module test files. Each file was restored with `git checkout` afterwards,
and `git status` was clean.

```sh
cargo test --bin browsey tmp_review_trash_folder -- --nocapture       # D1
cargo test --bin browsey tmp_review_batch_rollback -- --nocapture     # D2
cargo test --bin browsey tmp_review_percent -- --nocapture            # D3
```

D4-D8 and the unconfirmed concerns come from reading the code at the listed
lines.

## Validity validation (2026-10-23)

Baseline rechecked: `899a747d159d3aae9b2e942c013d3c23c95ae211`.
Purpose: assess each finding before creating issues or implementing fixes.
The preceding sections retain the original review and its original evidence
labels; this section records the subsequent validation.

### Findings and priorities

| ID | Verdict | Priority | Additional evidence and qualifications |
| --- | --- | --- | --- |
| D1 | Confirmed | High | A directory containing a symlink, a FIFO and a directory containing a mode-000 child all fail before the trash backend is called. The permission case ran as an unprivileged user. Full pre-copy, hashing/sync and between-item-only cancellation follow from the implementation. No large-file benchmark or physical USB test was performed. |
| D2 | Confirmed | Medium | Both later-item failure and cancellation leave the first item's actual bytes in two places: the restored source and an owned simulated trash directory. The retained trash item has a staging name. This check used real filesystem renames into a generated directory, rather than relying only on the existing fake backend's catalog. |
| D3 | Confirmed | Medium | All four percent-containing paths in the original finding produce altered arguments when passed through the real parser. The `%i` example becomes a different pathname. This can direct an editor to another file, although the parser itself does not modify file contents. |
| D4 | Confirmed | Medium | Tests using the real activity controller and selection-action controller reproduce both directions: a newer trash operation removes an older paste's progress/Cancel, and an older trash completion unregisters a newer paste's listener and hides its activity. File-service calls were mocked; no real user files were transferred or deleted. |
| D5 | Confirmed | Medium | For each of Recent, Starred, Network and Trash, deferred replies overwrite a newer directory, append stale history after Back, clear a newer pending load's indicator, and publish an old error after newer success. Tests account for the loading store's 150 ms delay and 180 ms minimum visible duration. |
| D6 | Confirmed | Low | A generated application with `Terminal=true`, launched through the real Linux launcher, receives no terminal. Its stdin is null and the launch has no terminal wrapper. No personal application was launched. |
| D7 | Confirmed | Low | The actual console launcher, in an isolated child with an owned fake terminal on PATH, leaves that terminal child in Linux process state Z. The diagnostic explicitly waits for that exact generated child afterwards. No existing desktop process was stopped or reaped. |
| D8 | Confirmed; broader than cosmetic | Low | `.bashrc` becomes `-1.bashrc`, losing its hidden-file naming. A generated valid TAR.GZ file is detected as `TarGz` before conflict renaming and as plain `Gz` after the name becomes `archive.tar-1.gz`, despite identical bytes. This changes extraction interpretation; it is more than an awkward-looking name. |

The Desktop Entry specification explicitly requires substitution text to remain
unexpanded, supporting D3 independently of the reproductions. Its `Terminal`
key also establishes the expected terminal-launch behavior for D6.
See the [Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html)
and [recognized keys](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).
Rust documents that dropping `Child` does not reap it; the observed zombie in D7
matches the [standard-library contract](https://doc.rust-lang.org/std/process/struct.Child.html).

### The three previously unconfirmed concerns

**Google Drive silent no-op: not established as described.** The proposed
explanation assumes that `backend moveid` uses the normal destination-exists
skip checks of `moveto`. In the upstream implementation inspected,
`copyOrMoveID` calls `operations.Move` with a nil destination object. That is a
different path from `MoveFile`/`moveOrCopyFile`; `--ignore-existing` alone does
not prove that the move is silently skipped there. The upstream `Move` contract
explicitly warns about duplicate creation or errors when an existing destination
has not been supplied. Consequently, destination races still warrant testing,
but the report's particular silent-skip outcome must not be filed as a confirmed
defect from those flags alone. Sources:
[Drive ID transfer implementation](https://github.com/rclone/rclone/blob/master/backend/drive/drive.go)
and [Move implementation and destination contract](https://github.com/rclone/rclone/blob/master/fs/operations/operations.go).
No rclone executable was available on this validation session's PATH, and no
configured account was inspected. Establish the exact runtime version and use
generated files in a dedicated demo account before deciding the actual result.

**FAT/exFAT case-only rename: plausible, still unconfirmed.** The special
temporary-name handling is restricted to MTP. That makes the concern credible,
but it does not establish the behavior of every FAT/exFAT driver and mount.
No FAT/exFAT filesystem was mounted or physical USB device used in this review.
Keep this as an investigation, with generated files on an explicitly scoped
filesystem; do not claim that a USB regression has been reproduced.

**Chained batch rename: confirmed order-dependent limitation.** A numeric
preview accepts `1.txt -> 2.txt`, `2.txt -> 3.txt` without reporting a conflict.
Applying that order fails safely because `2.txt` exists. Applying the reverse
dependency order succeeds and preserves both files' contents. This is a real
preview/apply inconsistency and a useful improvement candidate, not evidence of
data loss. Treat it as low priority unless dependency-aware batches are part of
an explicit product contract. Cyclic renames were not tested.

### Documented behavior and fix constraints

Retaining local permanent-delete backups without automatic expiry is documented
behavior. It should not be filed as a newly discovered deletion defect, or
changed to irreversible deletion as part of an unrelated fix. The disk-space
and cross-filesystem costs remain relevant usability/design considerations.

The recommended D1 fix needs an undo/recovery design before implementation.
Removing the pre-copy without replacing its fallback changes an existing safety
property. Preserve source ownership checks, avoid following symlink targets,
and handle an unidentifiable trash entry or failed rollback explicitly. Do not
relax the shared copy engine's symlink and special-file guards globally merely
to make trashing succeed. D1 and D2 should be designed and verified together.

D4 contains two related failures: ownership is lost when another activity
starts, and unscoped completion cleanup can remove an unrelated current owner.
Scoping all cleanup calls addresses the second failure but does not restore an
older operation's progress or Cancel. A fix must also define how overlapping
activities are retained, restored or refused. Avoid claiming that adding event
names to cleanup calls alone resolves the whole finding.

Recommended order: D1/D2 together, D3, D4/D5, then D6-D8. Keep the Drive and
FAT/exFAT investigations separate from confirmed defects. The chained-rename
limitation can be tracked as a smaller preview/behavior improvement.

### Validation evidence and workspace state

- 10 Rust diagnostic tests passed, including the isolated console child check,
  percent-path parsing, unreadable/symlink/FIFO inputs, physical duplicate
  payloads after rollback, archive classification and chained rename ordering.
- 14 frontend diagnostic cases passed: 12 special-view race cases and two
  overlapping activity-controller cases.
- These diagnostics assert the observed faulty baseline behavior. Their passing
  result establishes reproduction; it does not mean the defects were fixed.
- All temporary Rust includes were removed and the six source files were
  restored byte-for-byte to their original hashes. Both temporary frontend test
  files were removed. No application logic, installation, settings, real trash,
  accounts or devices were changed. No issues or PRs were created.
- Diagnostic sources and successful logs are retained locally under the ignored
  `target/defect-review-validation-2026-10-23/` directory. The final logs are
  `rust-validation-complete.log` and `frontend-validation-complete.log`.
- This follow-up does not certify the unrelated areas listed as reviewed without
  findings, or replace full native/platform acceptance.


## Implemented fixes: 2026-10-23

Implementation branch: `fix/defect-review-2026-10-23`, based on `899a747d`.
The original review and validity assessment above remain historical evidence.
The changes below fix the confirmed behavior; they do not promote the unconfirmed
Drive or FAT/exFAT concerns to confirmed defects. No substantial UI/layout change
was made.

| Finding | Implemented behavior | Regression evidence |
| --- | --- | --- |
| D1 | Trash stages and delegates to system trash without a full central backup copy. Undo uses the actual retained payload. A delayed catalog produces a deferred undo action rather than another data copy. | Tree with a relative symlink, unreadable child and FIFO; 8 GiB sparse-file guard; real Linux trash in a child with private HOME/XDG; delayed catalog and repeated undo/redo. |
| D2 | Failure and cancellation restore actual trash payloads and remove their metadata. Occupied original paths are preserved, blocked payloads remain recoverable and their undo actions remain retryable. | Physical fake-trash payload checks for failed/cancelled batches, no duplicate catalog entries, occupied-path preservation and successful retry. |
| D3 | Exec field codes are expanded once from the original template; inserted paths, application names and literal percent signs are opaque. | Literal `%i`, `%d`, `%n`, `%%`, combined field-like text and spaces remain exact command arguments. |
| D4 | The existing single visible progress panel retains a separate listener, progress, timer and Cancel state for every overlapping operation. Completing or hiding newer work restores older active work. | Both completion orders through the real activity controller plus selection actions; background updates, cancellation, delayed registration, same-event restart, stale timers and cleanup of all listeners. |
| D5 | Recent, Starred, Network and Wastebasket reject results/errors/loading cleanup from superseded requests. Network setup finishes before publishing its location, avoiding cancellation by the UI's search-reset reaction. | Twelve stale-result/history/loading/error cases across all four views, plus the network subscriber/search-reset case and the existing saved-connection browser test. |
| D6 | Linux terminal desktop entries use GIO's terminal-aware desktop launcher, with a typed launch error when a terminal is unavailable. | Fresh child with private desktop entry and fake terminal wrapper; literal target arguments and missing-terminal failure. No real terminal or user associations used. |
| D7 | Console launches use the existing waited-child pattern shared with Open With. | Private fake terminal through the actual console launcher; its PID disappears after exit rather than remaining a zombie. |
| D8 | Conflict naming preserves dotfile stems and supported compound archive suffixes, consistently for local and cloud candidate names. | Dotfiles, Unicode and compound suffixes; a generated renamed tar.gz still detects as TarGz and extracts its original document. |
| Chained batch rename | A complete dependency order is planned before mutation. Acyclic chains work in either request order; replies retain input order and undo/redo use the execution order. Cycles fail before changes. | Both chain orders with independent byte checks and undo/redo; cycle refusal with unchanged sources. Existing external-conflict rollback tests retained. |

### Trash ownership and recovery details

The shared copy engine's symlink, special-file and ownership guards were not
relaxed. Root/parent path checks remain active. Same-filesystem trashing moves
the directory entry, so symlink descendants are retained without following them;
top-level symlink policy is unchanged. A native backend may still refuse an
unsupported filesystem or trash operation.

Catalog entries are indexed by original path. Pre-existing IDs for the same
original path are excluded, and ambiguous new matches are refused rather than
selecting arbitrary trash contents. Deferred actions retain those exclusions.
Once resolved, a deferred action becomes the ordinary Move action before a
restore attempt, so a blocked destination can be retried even after its restore
metadata has been rewritten.

The staged-path journal now stays until the original restore metadata is
rewritten or rollback succeeds. Startup can repair metadata for a payload
already in trash. Tests isolate their journal under a process-specific temporary
directory. Native trash tests use a separate HOME/XDG child and never inspect or
mutate the user's real trash. A retained trash entry is not a second immutable
backup, and externally purging it can make Undo unavailable.

Permanent deletion and retained-backup recovery keep their documented semantics.
Cloud operations do not acquire local undo. Archive formats and security guards
are unchanged.

### Verification

The passing checks and native run identifiers are recorded below. Local command
logs are retained under ignored `target/defect-review-fixes-2026-10-23/`.

- Rust workspace, all targets/features: **980 application tests and one GLib regression passed**; 20 pre-existing opt-in tests remained ignored.
- Frontend: **898 tests passed** across 85 files.
- Browser: **144 Playwright tests passed**. An earlier run exposed the Network
  setup/search-reset interaction; it was fixed and covered by a regression test.
- Rustfmt, Clippy with warnings denied, ESLint, Svelte/TypeScript checks and
  frontend production build passed.
- Blocking Semgrep: **0 findings**; four rule/exit-code regression tests passed.
- Native harness policy: **173 tests passed**. Dependency policy, vendored
  hashes, PDFium provenance and the backend error-hardening guard passed.
- Documentation lint/check/build, **18 documentation tests** and all **20**
  strict consistency checks passed.
- Private native Open With and accessibility: **PASS**, run
  `e7e67903-d35b-4f3a-a056-b3407cdb897d`; owned candidate/driver exited and
  namespace supervisor teardown was confirmed.
- Private native clipboard/trash and accessibility: **PASS**, run
  `353df8f6-b950-4b0f-a228-94c15fba4611`; independent effects and owned
  candidate/driver teardown passed. The initial setup-only run
  `a639403a-1f37-401c-ab3b-32d233c6c0dd` was **BLOCKED** by missing `xclip`,
  before app launch. Its report is preserved; only the isolated test dependency
  was added under `target/native-tools/`, with no system package installation.

Native checks cover local Linux fixtures and private desktop services only.
They do not certify physical USB/phone behavior, real cloud accounts, Windows,
macOS, power-loss durability or installed-release acceptance. No installation,
merge or publication is implied by these test results.

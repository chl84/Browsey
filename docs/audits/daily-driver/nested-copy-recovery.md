# Nested Copy Cleanup and UI Recovery

Date: 2026-10-03
Baseline: `7ab7612`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Incremental implementation; overall Priority 0 remains open.

## Confirmed Regressions

Three disposable-fixture tests failed against the preceding implementation:

- A failed nested copy recursively removed an untracked file added by another
  process to the destination.
- The same recursive cleanup removed a completed copy subsequently edited by
  another process, even though the edited file was no longer unchanged output.
- Replacing the destination directory redirected recursive cleanup to a
  competing directory and removed its contents.

Three mocked frontend regressions also failed before correction: local paste
failure did not reconcile the listing; a successful move was reported as failed
when only listing refresh failed; and combined operation/refresh failures did
not retain both signals. No native installed UI reproduction is claimed.

## Implementation

`clipboard::OwnedCopyPaths` records directories and successful local file copies.
File identity/version is captured from the open output handle, not from a later
path lookup. Cleanup removes only unchanged registered files and then empty
registered directories. It checks registered ancestor identities as well as
leaf ownership. It never recursively removes untracked contents. Errors retain
the original operation error and report paths left behind or cleanup failures.

Directory identities are indexed by path; ancestor lookup is proportional to
path depth rather than scanning every recorded directory for every file. On
Linux, existing no-follow permission restoration lets cleanup remove owned
children of directories whose source mode was restored before another failure.
Directory finalization applies permissions through a verified directory handle.

GIO continues to handle GVfs transfers. Because GIO owns its output handle,
these outputs are not recorded as safely owned from a later path lookup. Failure
cleanup leaves them in place and reports nonempty directories. No new native
GIO acceptance evidence is claimed.

`fs_utils::FileState` combines stable identity, size and modification time, plus
Unix change time. `TreeSnapshot` records native path names, directory identities
and regular-file versions before a copy/delete move fallback. Both move engines
recheck the tree before source deletion, retaining sources and completed copies
on changes or verification errors. Renames that succeed atomically do not scan
the tree. Clipboard scanning checks cancellation between entries.

Additional tests cover edited regular/nested move sources in both engines,
added/removed/renamed children, equal-length edits with changed timestamps, and
cancellation during the metadata scan. No timing sleeps, disk filling, actual
media disconnection or personal files are used.

Local paste now attempts refresh after operation failure, keeps the clipboard,
preserves the original error, and adds F5 guidance when refresh also fails.
Successful transfers remain successful if only refresh fails; a successful cut
still clears its original clipboard selection. Progress-listener cleanup failure
cannot suppress error feedback or reconciliation. There is no automatic retry.

## Verification

- `CARGO_NET_OFFLINE=true bash scripts/maintenance/test-backend.sh` passed
  resource/vendor integrity, four dependency-policy tests, formatting, cargo
  check, Clippy with warnings denied, typed-error guard and the backend suite.
  Semgrep is unavailable; advisory/blocking runs were skipped, not passed.
- After indexing ancestor ownership checks, the final `cargo fmt --all`,
  `cargo clippy --offline --locked --all-targets --all-features -- -D warnings`
  and `cargo test --offline --locked --all-targets --all-features -- --quiet`
  passed: **565 backend tests**, three opt-in native tests ignored. Eight new
  backend test functions cover this batch.
- Final frontend lint/naming and Svelte/TypeScript checks passed with zero
  Svelte errors/warnings. Unit tests passed **275 tests in 37 files**, including
  four new paste-recovery regressions.
- All **55 browser tests** and the production frontend build passed. Browser
  tests use mock IPC and are not native device/platform acceptance.
- `git diff --check`, all 20 strict documentation consistency checks and all
  18 checked local documentation references passed before committing.

## Explicit Limits and Next Work

- This guard covers failed directory-copy cleanup, not automatic batch rollback
  of previously completed `Action::Copy` entries or later history undo. Those
  ownership boundaries remain an explicit next task.
- Metadata detects ordinary edits but is not a content hash. In-flight output
  edits, timestamp restoration/coarse filesystems, and changes after a final
  check are not an atomic consistency guarantee. Device loss and power-loss
  durability remain separate acceptance work.
- Fallback snapshots add two metadata walks and entry-proportional memory.
  Cancellation is checked between clipboard scan entries, not inside blocked
  kernel I/O. Very large trees and real MTP/network media need performance and
  native acceptance before claiming equivalent responsiveness.
- Windows shared code is not runtime-tested here. Physical devices and fresh
  Fedora/Ubuntu installed builds remain open in the parent plan.
- UI tests use mock IPC. They verify reconciliation calls, preserved messages,
  clipboard behavior and no automatic retry, not actual mounted-device recovery.

No dependency changes, version bump, publication, installation or real-device
formatting is part of this batch. Commit/push follows the requested workflow.

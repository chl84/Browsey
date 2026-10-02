# Daily Driver File Safety Baseline

Date: 2026-10-03
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Initial Priority 0 implementation; not complete platform acceptance.
Source baseline: `7dbf39c` plus the uncommitted changes described below.

## Scope and Support Matrix

The execution follows the plan's Linux-first scope. Only Omarchy is available
on this machine. The other rows remain validation targets, not new guarantees.

| Target | Current evidence from this run | Still required |
|---|---|---|
| Omarchy 4.0.4, Hyprland, Wayland | Real Rust filesystem/codec tests with disposable fixtures; frontend unit/browser tests | Installed-build manual acceptance, physical USB/MTP and network scenarios |
| Fedora, GNOME Wayland | Existing support target; no fresh run here | Clean RPM install/upgrade and manual workflow checks |
| Ubuntu LTS, GNOME Wayland | Existing support target; no fresh run here | Clean DEB install/upgrade and manual workflow checks |
| Windows | Maintenance-only scope; shared code was changed but not run on Windows | Windows compile/runtime validation before claiming parity |
| macOS | Outside current support scope | Separate approved support plan |

No personal files, physical devices, account data, or installed applications
were modified by these tests. They are not clean-distribution package tests.

## Existing Coverage Inventory

The initial backend run passed 530 application tests with three opt-in tests
ignored. Inventory uses current code and tests rather than reopening completed
Linux 1.0 audits.

| Family | Existing evidence | Limits and follow-up |
|---|---|---|
| Local copy/move | `src/clipboard/tests.rs`: conflicts, source/destination disappearance, permissions, directory merges, mid-batch cancellation and rollback | New cases below close concrete atomic-move gaps; concurrent tree changes still need broader acceptance |
| Rename | `src/commands/rename/mod.rs`: conflict/no-overwrite, missing source, permission denial, duplicate inputs, batch rollback, undo/redo, symlink rejection | Installed UI and unusual-name acceptance remain separate |
| Delete | `src/commands/fs/delete_ops.rs`: backups, undo, batch failure, cancellation, permissions, item progress | Cancellation rollback failure needed the correction below |
| Trash | `src/commands/fs/trash/tests.rs`: injected backend failures, restore/purge events, staging recovery, native filename encoding | Fake backend evidence is not a real desktop trash run |
| Archive creation/extraction | `src/commands/compress/tests.rs`, `src/commands/decompress/regression_tests.rs`, `src/commands/decompress/util.rs`: real codecs, passwords, permissions, hostile entries, budgets, cancellation, buffered-write/read failure injection | Device loss and kernel/decoder cancellation latency are not fully reproduced by fixtures |
| Undo | `src/undo/tests.rs`: apply/undo/redo, batch rollback, existing targets, snapshot identity | New history-cap/redo checks below; no persistent history is claimed |
| Session cleanup | `src/undo/backup.rs`: cross-process lock ownership and age-based cleanup | New killed-process test below; this proves cleanup, not recovery of in-memory undo after restart |
| Mixed transfers | `src/commands/transfer/execute/tests.rs`: fake rclone, partial copy/move, cancellation during active transfers, conflict checks, cache invalidation | Not real provider quota/authentication or physical network acceptance |
| Frontend workflows | 271 unit tests and 55 browser tests cover dialogs, errors, drag/drop, clipboard snapshots, USB/MTP UI and settings | Browser tests mock Tauri IPC; they cannot establish native/device correctness |

## Confirmed Defects and Corrections

1. **Late destination conflict during local move.** A test created a competing
   target between the metadata check and rename. The original code succeeded
   and overwrote it. Clipboard moves now reuse the undo engine's native
   no-replace rename and return a conflict instead of falling back on every error.
2. **Already cancelled move still executed.** The initial regression failed
   because the source was renamed despite a set cancellation token. Check
   cancellation before rename and again before fallback source deletion.
3. **Only complete copy removed after partial source deletion.** A real fixture
   allowed deleting source-directory contents but denied removing its root.
   The old undo fallback then deleted the destination, losing the fixture's
   contents. Preserve the destination and report its path on source-delete failure.
4. **Cancellation concealed failed delete rollback.** Recreating the deleted
   source name blocked restoration, but the old code returned ordinary
   cancellation and discarded the rollback error. Report `delete_failed` with
   rollback/recovery guidance; preserve the competing file and undo backup.
5. **Unchecked fallback writeback.** Code review found no final sync before the
   undo move fallback deleted its source. Finalize through the open destination
   handle and propagate sync errors. Injected final-writeback tests verify error
   propagation and no access to pre-existing destination files. This is not a
   guarantee against power loss across an entire directory transaction.
6. **Special-file input could block before validation.** Copy and undo opened
   input files without the nonblocking regular-file guard already used by ZIP
   creation. Reuse one helper in all three paths. Unix FIFO/socket and symlink
   tests verify rejection before creating a destination; Windows opens reparse
   points without following the leaf, but has not been runtime-tested here.
7. **Source identity not revalidated by clipboard move.** Reuse existing
   snapshots before rename and fallback deletion. A deterministic source-replacement
   regression verifies that neither the replacement nor the saved original is moved.
   This narrows the race window; it does not make a mutable tree transactional.

The first four regressions were run against the original implementation and
failed before their fixes. The remaining findings came from code review and
have targeted regression coverage. Tests use injected boundaries or real
fixture permissions, not timing races or personal data.

Additional coverage verifies the 50-action undo cap, redo invalidation, fresh
manager history, redo conflict preservation/retry, Unicode/non-UTF-8 child names,
and OS-lock release when a test subprocess is killed. The helper subprocess
creates only its own temporary session; another live session remains protected.

## Verification Commands and Results

Toolchain: Rust 1.98.0. No dependency upgrades were performed.

- Initial baseline: `cargo test --offline --locked --all-targets --all-features -- --quiet`
  passed 530 tests; three opt-in native tests were ignored.
- `CARGO_NET_OFFLINE=true bash scripts/maintenance/test-backend.sh` passed resource
  integrity, four dependency-policy tests, formatting, cargo check, Clippy with
  warnings denied, typed-error guard, and the backend suite. Semgrep is not
  installed and both its advisory/blocking runs were skipped, not passed.
- `cargo fmt --all` and `cargo clippy --offline --locked --all-targets --all-features -- -D warnings`
  passed. The final `cargo test --offline --locked --all-targets --all-features -- --quiet`
  passed 546 tests; three opt-in tests were ignored in this ordinary run.
- `npm --prefix frontend run lint` and `npm --prefix frontend run check` passed;
  zero Svelte errors/warnings.
- `npm --prefix frontend run test` passed 271 tests in 37 files.
- `npm --prefix frontend run test:e2e` passed all 55 browser tests with mock IPC.
- `npm --prefix frontend run build` passed.
- `timeout --signal=TERM --kill-after=3s 60s dbus-run-session -- env BROWSEY_PRIVATE_TEST_BUS=1 cargo test --offline --locked waits_beyond_old_timeout_and_reports_real_job_progress -- --ignored --test-threads=1 --nocapture`
  separately passed the opt-in USB contract test in 26 seconds. It uses a fake
  UDisks service on a private session bus, not real USB or the system bus. Native
  graphical integration tests remain unexecuted here.
- `git diff --check` passed. `bash scripts/maintenance/check-docs-consistency.sh --strict`
  passed all 20 checks; a separate local-reference check found all 21 checked
  documentation references valid.

## Remaining Priority 0 Work

- Complete installed-build, physical-device and cross-distribution acceptance.
  The general copy/rename/delete/extract validation row stays unchecked.
- Extend fault-injection evidence to actual local transfer write/finalization
  paths for disk-full and disappearing media, rather than substituting the
  extractor's writer tests for every operation.
- Review source/destination replacement and content changes throughout nested
  copy, cleanup and rollback. Current root identity checks do not freeze trees
  or protect every concurrent edit to an existing file.
- Verify user-facing partial-state refresh and safe recovery after real I/O
  failures. Retained copies are deliberate; automatic retry/deletion is unsafe.
- Keep local history session-only. Startup can prune abandoned undo sessions;
  do not promise persistent undo or automatic resume based on the cleanup test.
- Validate shared changes on Windows and run native integration/Semgrep gates
  in the appropriate environment before publication.

Priority 0 and the overall plan remain open. No version bump, commit, push,
release, installation, or real-device formatting has been performed in this run.

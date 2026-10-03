# Recovery Storage Measurements and Policy

Date: 2026-10-03. Starting source: `53ef748`, plus the measurement and diagnostic
changes in this increment. Scope: disposable local Linux backups, not cloud
working copies, thumbnail caches, logs or maintenance-agent build caches.

The measurements confirm that neither logical file size nor the 50-action limit
is a storage budget. The safe policy is to provision for retained backups and
the next full recovery copy, preserve protected/live data regardless of age,
and avoid automatic quota eviction. Settings now additionally reports
filesystem-allocated bytes where the platform exposes them. This remains a
bounded, read-only diagnostic, not a reclamation or admission-control feature.

## Reproduction

The opt-in test uses the production action, backup, verification and history
engines. It creates uniquely named disposable fixtures and its own undo storage;
it never scans the user's recovery directory. Run it alone:

```sh
PATH="$HOME/.cargo/bin:$PATH" cargo test --locked --offline recovery_storage_workloads -- \
  --ignored --exact undo::tests::measurements::recovery_storage_workloads --nocapture
```

To measure another filesystem, set `TMPDIR` to a newly created empty directory
on that filesystem. Do not point `BROWSEY_UNDO_DIR` at personal recovery data.
The test prints versioned JSON and cleans up only its generated fixture trees.
It is opt-in because it writes about 200 MiB over repeated round trips. A panic
can leave its isolated backup fixture for inspection; it cannot purge real data.

## Observed Storage

Environment: AMD Ryzen 5 7520U, 8 logical CPUs, approximately 15 GiB RAM,
Linux `7.2.5-3-omarchy`, x86_64. Two standalone debug-test runs used `/tmp` tmpfs
and a disposable directory on the home Btrfs filesystem. The generated data is
deterministic, including a zero-filled sparse file; it is not a model of every
user workload. Allocation below is `st_blocks * 512`, including directory blocks
in the measured tree, not exclusive CoW/compression-aware physical usage.

| Generated source | Logical contents | Source allocation | Backup allocation |
| --- | ---: | ---: | ---: |
| One regular file | 32 MiB | 32 MiB | 32 MiB |
| 1,024 eight-byte files | 8 KiB | 4 MiB | 4 MiB |
| 128 nested 8 KiB files | 1 MiB | 1 MiB | 1 MiB |
| One sparse file | 32 MiB | 4 KiB | 32 MiB |

Both filesystem runs reported these allocation values. Sparse preservation is
not supported by the current verified userspace copy path. Tiny files can have
hundreds of times their logical size in allocated blocks. Filesystem metadata,
snapshots, shared extents and later writes make these counts insufficient to
predict exact free-space consumption.

Three undo/redo round trips reused each existing copy backup without increasing
its measured size. Clearing history left those backup files on disk. A separate
51-delete-action workload retained only 50 undoable actions, but the evicted
action's 128 KiB backup remained. Creating a real recovery marker and dropping
its protection object retained protection; the read-only inventory reported one
marked session. Existing separate-process cleanup tests remain the evidence
that abandoned marked sessions and locked live sessions survive cleanup.

Single first-undo observations were 67/227/45/64 ms on tmpfs and 83/1,716/279/97 ms
on Btrfs for the table's four shapes. These are not medians, performance budgets,
installed-app timings or power-loss durability results.

## Safe Budget and Retention

For planning, count all retained recovery and working data separately from
rebuildable caches. Reserve room for the next full backup in addition to the
ordinary destination write. Use logical size for sparse inputs and allocated
size for small-file trees, whichever is greater; add directory/metadata overhead
and leave normal filesystem operating headroom. Concurrent operations and other
applications also need space. A batch counts as one history action and can contain
arbitrarily large trees, so multiplying by 50 is not an upper bound.

There is no evidence for a universally safe fixed GiB quota or expiration age.
The current conservative retention decision is therefore:

- Never evict marked sessions, locked live sessions, cloud working copies or
  files an open program may still need to meet a budget or age target.
- Preserve existing startup cleanup of verified abandoned unmarked sessions.
  Do not weaken its lock/marker checks or treat clearing history as permission
  for a new deletion mechanism. Legacy and unknown directories stay untouched.
- Treat incomplete diagnostics as lower-bound observations, not complete totals
  or proof of available capacity. Free space and allocation can change during
  an operation; preflight estimates cannot guarantee completion.
- Under storage pressure, finish operations and inspect retained recovery data.
  Recover and verify required files before any explicit manual cleanup. Do not
  clear markers merely to make the storage number smaller.

A future configurable hard quota would require an explicit decision about
refusing new operations safely, not silently discarding recovery data. Byte/age
settings and automatic recovery deletion are deliberately not introduced here.
Existing thumbnail cache eviction and log rotation remain separate mechanisms.

## Verification Boundaries

Focused tests cover block-based accounting, null allocation on unsupported
platforms, partial scans and frontend compatibility with older summaries.
Installed recovery UX, physical-device failure and full manual recovery remain
in the [acceptance checklist](../../operations/linux-release/daily-driver-validation-checklist.md).
No installation, application restart, account mutation or release is part of this
increment. Strict maintenance passed 645 backend tests (8 opt-in tests ignored),
318 frontend tests, 62 browser tests, warnings-denied Clippy, clean Semgrep,
production frontend build and 20 documentation checks. The two opt-in storage
runs passed separately. Results are also recorded with the TODO checkpoint.

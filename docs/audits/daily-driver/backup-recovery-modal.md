# Backup recovery modal

Settings > Stored data presents a short backup status, Show all, Delete all,
and collapsed Advanced details. Refresh is available inside the backup modal.
Delete all uses the shared destructive confirmation, explicitly clears undo and
redo history, and deletes stored data only under verified session locks. Other
instances' locked sessions and unsafe/unreadable sessions are retained and
reported. Complete local backup-producing operations, rollback, and recovery
exclude maintenance, including gaps between allocation and history recording.
Owned session directories and their lifetime locks remain for new allocations;
abandoned sessions and their locks are removed. Legacy folders stay excluded.
The storage summary refreshes automatically after deletion or closing Show all.
Recovery lists backup root
entries by filename, regular-file content size and modification time. A small
folder browser reuses local listing; paths can also be pasted and opened with Go.
Both the backup list and fallback folder picker use the shared ModalShell,
modal spacing/density tokens, button styles, and error presentation. The folder
path uses TextField; recovery progress uses ProgressBar.
Verified success shows “Recovered <name>” beside Open folder; the full output
path is available in the message tooltip. A typed occupied-original reason opens
the folder picker with neutral guidance. Other errors retain their diagnostics
and error styling, including failed copies with uncertain output paths.
Escape uses the shared flow: leave a focused text field first, keeping focus in
the recovery dialog, then close recovery while Settings stays open. During
recovery, Escape does not dismiss the dialog; cancellation uses Cancel.

After verified recovery, a private fixed-size sibling status record marks that
backup as recovered. The backend persists and verifies it under the same session
lock/read lease before returning success; the frontend then removes only that
row. Refresh and process restarts honor the status. Backup bytes and protection
markers remain available to undo/redo. Failed/cancelled copies never write a
success record. Status-write failures report the verified output path and keep
the row listed, without redirecting to another recovery destination.
The status is tied to the root/bucket/session version and the verified ordered
tree metadata fingerprint. A fully measured mutation becomes pending again;
partial measurements preserve the last completed status for the same root
version. Malformed or unsafe status files never hide a backup.

Stored backups survive closing and restarting Browsey without an automatic
expiry, regardless of recovery markers. Startup cleanup acquires the abandoned
session's lock and uses nonrecursive directory removal only: any remaining entry
prevents deletion, including a zero-byte file or an empty backed-up folder.
Marked or uncertain sessions retain their diagnostics. Recovery does not clear
markers or persist undo/redo history. The 50-action history limit is not a storage
limit. Existing diagnostics remain bounded and read-only.

The list's **Recover** button first copies the stored root to its original path.
New backup allocations write a versioned private sibling `.origin.json` record
before any backup copy/move, with the backup name and absolute original path.
The record is separate from backup contents and bounded to 16 KiB when written
and read; malformed, unsafe or unavailable records cannot select a destination.
**Recover to…** opens only when the original destination is unavailable or its
copy fails. Occupied original paths (including folders and dangling symlinks)
are never overwritten or given alternate names automatically. Missing parents
are not recreated. Cancellation and source lock/version failures do not trigger
the folder picker. A failed original copy reports its uncertain output path;
copying to another folder requires an explicit user action.
Marker contents are diagnostic text, not trusted destination paths or a journal.
The listing uses bounded metadata traversal without following symlinks. Completed
backups in the current process can be recovered while Browsey remains open.
Ownership is established from the in-process session registry and verified
directory/original lock identities, not from the PID in a session name. A
per-bucket access registry distinguishes active writes from the lifetime retention
lock: recovery holds a read lease through streaming and verification; backup
copy/move/deletion primitives and empty merge backups register their writes,
which wait while a recovery reader is active. Nested writers use counted guards,
and unrelated buckets remain available. Another process's running sessions
remain unavailable and must be recovered in the owning instance.
A restore resolves a three-component relative
reference below the validated backup root, acquires the existing sibling session
lock for abandoned sessions and revalidates a version tied to the root, bucket
and session identities. Only root metadata contributes to the version so new
backups/markers elsewhere in a live session do not invalidate an existing row.
The lock remains held through all writes and verification, blocking startup
cleanup. No missing lock is created or reconstructed.

The destination must be an existing absolute local folder outside recovery
storage, with no symlink ancestors or parent components. In **Recover to…**,
naming adds a numbered suffix only on a real conflict, including dangling
symlinks. The existing local
paste engine is reused in receipt-required mode: bounded chunked copying, shared
byte events, cancellation, exclusive target creation, sync and BLAKE3 readback.
Source and output trees are checked before returning success. This does not
freeze external writers, reconstruct an interrupted operation or guarantee that
the stored backup was complete before recovery. On error/cancellation, source
and markers remain and any uncertain partial output is retained and reported.

The modal uses its own instance of the shared activity controller and ProgressBar
so it does not replace another explorer operation. Command replies own completion
and success; child file completion never dismisses the recovery task. Destruction
requests cancellation and releases the listener when the command completes.

Disposable backend fixtures cover files, nested/empty directories, repeat naming,
original-location recovery after restart, occupied/missing original destinations,
write failure/cancellation and unsafe origin/destination links,
removal from the pending list after verified recovery, durable status after
restart, changed nested backup contents, status failure and continuing undo,
unmarked backup survival after process termination,
locks held during writes and readback, completed current-process image recovery,
per-bucket busy/idle transitions, unrelated backups, mutations waiting for
recovery, foreign in-use sessions, traversal, stale references,
symlink inputs/locks/destinations, dangling name conflicts, cancellation and failed
verification. Frontend tests cover progress, completion, cancellation, stale scans,
listener lifecycle, keyboard access and narrow layouts. Browser tests use mock
data; this increment does not claim installed-build recovery acceptance.

# MTP phone discovery

Browsey starts GIO's volume monitor on the GTK main thread and maintains a
thread-safe snapshot of MTP devices. Only snapshots cross into mount-list workers.
Volume/mount add, remove and change events refresh the sidebar. UDisks monitoring
is retained for block devices. Merely discovering a phone never mounts it.

Clicking an unmounted phone mounts the exact GIO volume and uses its authoritative
local GVFS path. There is no prefix-based fallback to another connected phone.
Concurrent requests for that phone are rejected/coalesced. A 30-second deadline
cancels an unresponsive connection and reports unlocking/file-transfer guidance.
Phones do not expose formatting or Unix ownership/permission mutations.

Automated checks: `cargo test`, `npm --prefix frontend test`, and
`npm --prefix frontend run test:e2e`. The MTP browser tests simulate hotplug,
mount success, locked phones, repeated clicks, unplug/replug and unplug during
connection. They do not mount or modify a physical phone.

File-operation safety boundary: a GIO-owned copy writer does not provide a safe
output ownership receipt. A copy/delete fallback move therefore may finish the
copy but refuse source deletion. Ordinary native rename is
unchanged when available. The error must retain the source and any remaining
output for inspection, without an automatic move retry. See the
[copy version/fallback report](audits/daily-driver/copy-version-validation.md).

GVFS copies use native GIO byte callbacks in either direction. Cancellation is
forwarded independently of those callbacks, and a failed/cancelled GIO copy is
not restarted using another writer. Aggregate file-content totals keep the task
open across multiple files; unavailable totals remain indeterminate. GIO may
retain partial output on failure or cancellation; inspect it before retrying.
Disposable local GIO tests do not replace physical-phone acceptance.

Manual acceptance on Linux with `gvfs-mtp` installed:

1. Close Files/Nautilus and start Browsey.
2. Attach a phone, unlock it and select File transfer/MTP. It should appear
   before opening Files, even when it was already attached at Browsey startup.
3. Click it; approve access on the phone if prompted. Check that the phone opens
   and remains a single sidebar entry. Its menu must not offer Format.
4. Disconnect while browsing: the entry disappears and Browsey returns Home.
5. Reconnect; also check a locked/rejected connection and retry after unlocking.
6. If available, connect two phones with the same display name; selecting each
   must open that specific phone, with no duplicate mounted/unmounted entries.
7. With disposable files only, check Copy and requested Move in both directions.
   A fallback writer without ownership evidence must refuse source deletion
   explicitly and retain both paths. Verify the result before any manual source
   cleanup; do not repeat an uncertain move automatically. This acceptance is
   separate from the automated opaque-writer fixture.

8. Open a large camera folder in Grid with Date or Size sorting. Allow metadata
   and thumbnails to finish and wait through several five-second refresh cycles.
   Existing image cards should not reshuffle or lose their loaded thumbnails.
   New files appear at the end; deleted files disappear. Use F5 or change sorting
   to reapply the order using available metadata. Scroll and select files during
   loading, and check that selection remains tied to paths.
9. Navigate elsewhere during a slow refresh or start a search. A late directory
   reply must not reopen the old folder or replace the search results.
10. Copy a large disposable file and a multi-file folder in each direction.
    Verify intermediate byte progress, then cancel a separate large transfer.
    It should stop without starting another copy, retain the source and refresh
    the destination. Inspect any retained partial output before cleanup.

Automated snapshot/metadata tests and simulated browser fixtures cover these
refresh contracts, not physical-phone or native-WebKit acceptance.

`RUST_LOG=browsey::mtp=debug browsey` logs discovery counts (not phone identifiers)
for diagnosis. Do not stop shared GVFS services while other applications use them.

API references: [GIO volume monitor threading and signals](https://docs.gtk.org/gio/class.VolumeMonitor.html),
[asynchronous volume mounting](https://docs.gtk.org/gio/method.Volume.mount.html).

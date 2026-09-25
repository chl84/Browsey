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

`RUST_LOG=browsey::mtp=debug browsey` logs discovery counts (not phone identifiers)
for diagnosis. Do not stop shared GVFS services while other applications use them.

API references: [GIO volume monitor threading and signals](https://docs.gtk.org/gio/class.VolumeMonitor.html),
[asynchronous volume mounting](https://docs.gtk.org/gio/method.Volume.mount.html).

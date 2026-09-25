# USB formatting regression checks

The normal Rust, Vitest, and Playwright suites cover job filtering, error classification,
duplicate requests, progress delivery, and the format dialog. They do not format devices.

## Long-running D-Bus contract test

Run this separately from the normal suite:

```sh
BROWSEY_PRIVATE_TEST_BUS=1 dbus-run-session -- cargo test \
  commands::network::usb_format::test_bus::waits_beyond_old_timeout_and_reports_real_job_progress \
  -- --ignored --exact
```

This test exports a simulated UDisks service on a private **session** bus, not the
system bus. It checks the actual GIO method signatures, waits 26 seconds for the
format reply, reads job progress, rejects a busy-device check, and resolves the
returned device property. It never invokes mkfs, mounts, or writes to a block device.
It also checks that `take-ownership` is a boolean set to `true` in the filesystem
options for ext4, btrfs, exFAT, and FAT32, including formatting without a label.

## Runtime behavior

- Destructive calls use GIO's `G_MAXINT` no-timeout value and wait for the method reply.
- Job queries are bounded separately; unavailable progress does not abort formatting.
- Percentages describe the **current job**, not an estimated percentage of the whole workflow.
- An uncertain reply never causes an automatic reformat. Reinspection checks jobs again.
- New ext4/btrfs filesystem roots belong to the calling user via UDisks
  `take-ownership`; this does not repair ownership on already-formatted media.
- USB **Properties** reuses the ownership/permissions dialog for the mounted root.
  Opening it does not change permissions, scan the drive recursively, or mount an
  unmounted volume. Hidden/rename controls are not offered for mount roots.
- Closing the client or losing its bus connection is not proof that UDisks stopped writing.
- Physical formatting and filesystem integrity still require separate, explicit testing
  on disposable media; the automated tests are not a hardware validation.

References: [GIO call timeout](https://docs.gtk.org/gio/method.DBusConnection.call_sync.html),
[UDisks job progress](https://storaged.org/doc/udisks2-api/latest/gdbus-org.freedesktop.UDisks2.Job.html).

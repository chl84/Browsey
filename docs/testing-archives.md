# Archive regression checks

Run `cargo test commands::compress` and `cargo test commands::decompress`.
These tests use disposable temporary files; no desktop session is required.

Coverage added after the September 2026 archive review:

- Final buffered-write failure is propagated before success, including small
  files whose contents fit entirely in a buffer. The shared copy helper flushes;
  the RAR callback separately checks its flush.
- Rollback removes only recorded outputs with matching identities and removes
  directories non-recursively. Untracked files and replacement files survive.
  Identity excludes mutable size/mtime: Unix device/inode, Windows volume/file ID.
- ZIP/TAR private and executable modes are restored without privileged bits.
  Directories remain writable during extraction and receive their final modes
  afterwards. Linux output creation uses 0600 files and 0700 directories.
  7z Unix-extension modes and Unix RAR attributes use the same permission policy.
- A small embedded, self-created 7z fixture with one empty `empty.txt` must produce
  a file, not an empty directory. It was generated with `bsdtar --format=7zip`;
  the tests do not require bsdtar to be installed.
- Real ZIP creation checks contents, empty/Unicode names, symlink targets,
  rejection of special files and lossy filename encodings, cleanup after early
  failure, and cancellation registered before collection begins.
- TAR preflight checks both declared payload totals and decoded stream bytes.
  The latter includes a bounded allowance for headers/padding (1024 bytes per
  allowed entry). Cancellation from a wrapped reader is not `Interrupted`, since
  `read_exact` and decoders can retry that error indefinitely.

Existing coverage also exercises ZIP64, TAR cancellation, RAR4/RAR5,
multi-volume RAR, password errors, batch failure/cancellation and extraction caps.

Limitations: filesystem/kernel I/O and opaque decoder header parsing cannot
always be interrupted immediately. Flush detects buffered-write errors but is
not an fsync/power-loss durability guarantee. Rollback is best effort if media
disappears or permissions change externally. POSIX mode tests run on Unix;
Windows DACL restoration is not implemented by these archive extractors.

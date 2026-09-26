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

## Password-protected archives

- Creating archives remains ZIP-only. The optional password enables WinZip
  AES-256 with a fresh random salt per file, including empty files and symlink
  payloads. File names and directory structure are not encrypted. Readers that
  support only legacy ZipCrypto cannot open these files; no weak-encryption
  fallback is offered for creation. Empty creation passwords are rejected.
- Extraction accepts ZIP AES/ZipCrypto, 7z AES and RAR passwords, including
  encrypted 7z/RAR headers. Passwords are passed through header inspection,
  destination planning and streaming extraction. A failed attempt rolls back
  before a retry. Authentication/checksum failures are never successful extraction.
- Missing and wrong passwords have structured error codes. Some formats cannot
  distinguish a bad password from corrupted encrypted data; the dialog says so.
  Destination-write errors and cancellation do not turn into password prompts.
- Batch extraction first handles the batch normally, then prompts separately for
  password failures only. Already successful archives are not re-extracted.
  Each successful retry is its own undo action; successful initial batch items
  retain the existing grouped undo action. Cancelling a password prompt stops
  subsequent retries, without undoing archives already extracted.
- Passwords are not stored in settings, history, logs or command-line arguments.
  They are not cached/reused across archives. Dialog values are cleared after
  submission and close. Browsey's owned Rust input strings and RAR wide-character
  buffers are zeroized on drop. JavaScript/IPC and decoder-internal copies cannot
  be guaranteed to be erased from memory; this is not protection against memory
  dumps, swap or a compromised desktop session. Embedded NUL characters are not
  supported; other whitespace and Unicode are preserved.

Tests generate ZIP and 7z archives at runtime and use attributed RAR fixtures.
Coverage includes missing/wrong/correct passwords, plaintext entries preceding
encrypted entries, encrypted headers, AES integrity failures, Unicode passwords,
stored/deflated ZIP files, empty files and symlink payloads. `sevenz-rust2`'s
writer is enabled only as a test dependency.

Run the frontend flow tests with:

```sh
npm --prefix frontend test -- src/features/explorer/modals/compressModal.test.ts src/features/explorer/file-ops/useExplorerFileOps.test.ts
npm --prefix frontend run test:e2e -- archive-password.e2e.ts
```

Browser tests use mock IPC and check the real dialogs, validation, masking,
keyboard submission, retries and clearing of inputs. Rust tests exercise the
actual archive codecs and filesystem operations separately.

Limitations: filesystem/kernel I/O and opaque decoder header parsing cannot
always be interrupted immediately. Flush detects buffered-write errors but is
not an fsync/power-loss durability guarantee. Rollback is best effort if media
disappears or permissions change externally. POSIX mode tests run on Unix;
Windows DACL restoration is not implemented by these archive extractors.

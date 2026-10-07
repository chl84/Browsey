# OneDrive acceptance and transport follow-up

Date: 2026-10-03. Scope: Unreleased changes after `d6e3c9f`, Linux/Omarchy,
rclone 1.75.1 and the approved empty `rclone://Onedrive/agent_test_folder`.
At the time of this run, Google Drive and Nextcloud had no approved disposable
folder. Later bounded Google Drive installed-build acceptance is recorded in the
[2026-10-07 production run](../../operations/linux-release/runs/2026-10-07-google-drive-production.md).
This OneDrive run is not
installed-app, release, quota, provider-web restore or cross-provider signoff.

## Isolation and cleanup

The opt-in tests require a non-root, existing, empty OneDrive parent and
`BROWSEY_TEST_CLOUD_WRITE_APPROVED=yes`. Each run creates a unique marked child
and a private local fixture. Successful cleanup reads the remote ownership
marker before moving only that child to normal provider trash, then checks that
the parent is empty. Failure retains test data; it does not silently retry,
delete personal files, empty the recycle bin or erase protected recovery.

The native candidate uses its own XDG profile and test-only password. The
accessibility helper verifies the exact candidate PID, executable and private
profile before every action. Keyboard input targets only its Hyprland window.
Authentication uses the existing rclone configuration in place; no credentials
are copied into the fixture or committed. Test-process RC daemons are stopped
on fixture drop, including failed tests. The installed Browsey is not replaced.

## Evidence collected

| Test | Observation | Boundary |
| --- | --- | --- |
| Working copy/archive backend | Persistent edits, same-size source change detection, unique upload, existing-target refusal, rename, encrypted ZIP and scoped trash | Pre-cancellation is not active cancellation |
| Archive tree backend | 32 files in eight groups, nested empty directories, encrypted ZIP, production mixed upload/download, exact bytes and retained originals; 120 s | Not a large/deep-tree performance budget |
| Active cancellation | Real positive rclone byte statistics before cancellation; cancelled result and exact source retained | Remote state inspected, not assumed rolled back |
| Process-scoped network fault | Real positive transfer statistics before a loopback CONNECT proxy drops only the test process's tunnels; network error and exact source retained | No desktop network disconnect, quota exhaustion or arbitrary proxy destinations |
| Native archive workflow | Context-menu compression, password checkbox, extraction password modal, real IPC/staging/OneDrive, refreshed output, exact bytes/empty directories and retained original folder/ZIP; 141 s | No external GTK drag receiver or ordinary installed-app signoff |
| Post-preflight competing destination | Same-size competing file created before CLI copy starts; production CLI and RC progress guards preserve its bytes and the source, rejecting no-transfer as success; 75 s | Not protection against a destination created after rclone's own check, nor provider CAS |

The first native test expected the ZIP stem as its output directory. Production
extraction instead uses the archive engine's returned root and a unique staging
suffix. The workflow had completed correctly; the corrected test discovers the
single new output directory without duplicating the naming policy, then checks
its contents and visible refresh. Failed test children were marker-verified and
moved to normal trash before retrying.

## Confirmed transport fixes

1. The CLI waited for child exit before reading stdout/stderr. A deterministic
   128 KiB output on each pipe reproduced a timeout. Independent readers now
   drain both pipes during execution. Retained buffers are bounded to 128 MiB
   stdout and 8 MiB stderr; readers continue draining after the limit. Overflow
   returns a typed task failure with unknown completion and destination-verification
   guidance, never successful truncated JSON or an automatic write retry.
2. Head-only shortening of lengthy failure output discarded the final cause.
   Failure feedback retains bounded head and tail. Signed HTTP(S) URLs are
   removed before truncation from CLI failures, adapter feedback and log previews;
   successful configuration/JSON output remains unchanged.
3. Mixed transfers had a separate, incomplete textual classifier and returned
   unknown errors for actual network loss. They now reuse the provider classifier
   and existing typed API codes. Real cancellation/network fault acceptance and
   focused mapping/privacy regressions cover the seam.
4. A deterministic prechecked-copy regression reproduced silent overwrite of a
   same-size competing file. The real OneDrive test also disproved the fake's
   assumption that `copyto --immutable` alone rejects that write. Single-file
   new-object copies now use `--ignore-existing --error-on-no-transfer`; directory
   copies retain their immutable/content-check guard. Progress uploads set
   `IgnoreExisting` per RC call and verify an increase in completed transfers,
   including zero-byte files. CLI exit 9 and unavailable/skipped RC completion
   are typed task failures with refresh/verification guidance, not claimed copies
   or automatic retries. These copy guards do not change explicit overwrite or
   move policy. The separate failure-path changes below prevent automatic retry
   of ambiguous/failed writes, including moves. The fake now models the actual
   single-file immutable limitation.

These guards reduce the preflight-to-transfer gap but cannot make a provider
write atomic. [rclone's documented skip/no-transfer flags](https://rclone.org/docs/#ignore-existing)
and [per-call RC configuration](https://rclone.org/rc/#setting-config-flags-with-config)
support the mechanism; the real-account byte checks above are its bounded proof.

### Follow-up failure-path hardening

The full provider rerun revalidated working copies/archives and active
cancellation, but the network privacy assertion found a legitimate apostrophe
inside an OData upload-session URL. That delimiter stopped the first URL scrub
early. The corrected regression covers the entire OData URL, JSON escaping and
case variations. Real revalidation on `ed109fb` passed: cancellation after
1,060,864 accounted bytes returned in 25.1 ms; process-scoped network failure
after 77,824 bytes returned in 50.2 ms. Feedback contained no signed URL, source
bytes remained exact and owner-checked normal trash left the parent empty.
The later three tests refused the nonempty test parent before writing. That is
ownership-guard behavior, not three independent provider/native regressions.
Only the marked failed child was moved to normal trash before retrying.

Code review also found completed failed RC jobs converted to generic I/O errors
and allowed CLI fallback, plus ambiguous responses after write submission. The
follow-up preserves a typed failed-job outcome and wraps post-write transport,
HTTP/JSON, cancellation and missing-job-ID failures as unknown completion. It
does not repeat either outcome automatically. Connect/startup failures before
the request remain eligible for guarded CLI fallback. Local Unix-socket fixtures
cover EOF, invalid JSON and server errors; output-limit fallback failed before
the correction. API and Debug/Display feedback reuse bounded signed-URL/JSON
secret redaction. RC response buffers also stop at 128 MiB instead of accepting
unbounded responses or partial JSON.

## Final verification checkpoint

The strict maintenance suite for `ed109fb` passed 666 backend tests (16 explicitly
opt-in tests ignored), 318 frontend tests, 64 browser tests, Clippy with denied
warnings, blocking/advisory Semgrep with zero findings and 20 strict docs checks.
The production Tauri candidate then built successfully in 4 minutes 6 seconds.
Its SHA-256 is
`137aaecab41381a84baf450ca807130563e6c3cd91760293037f825fe59ec1d9`.

All five real-provider tests passed serially against the approved empty parent
in 809.56 seconds, with owner-checked normal trash cleanup after each test:

- Working copies, same-size source changes, explicit unique upload, rename and
  password archives were revalidated.
- Active cancellation/network loss passed the corrected signed-URL privacy
  assertion, as recorded above.
- Native context-menu password ZIP creation and password-modal extraction used
  this final candidate, including the RC completed-transfer/no-retry changes.
- The competing-destination test preserved both source and competing bytes for
  CLI and RC progress. A legitimate zero-byte progress upload succeeded and was
  verified by remote stat and an empty download.
- The archive tree used 512 files in eight groups with depth eight and nested
  empty directories. Encrypted archive upload/download took 10.1/13.8 seconds;
  extracted-tree upload/download took 207.0/77.5 seconds, without changing normal
  transfer limits. All returned file bytes and the unchanged original ZIP were
  checked; copying into the occupied tree was refused.

This final checkpoint supersedes the earlier binary limitation for native
acceptance, not the scope boundaries. No installed-app replacement, remote root
write, global purge, quota exhaustion or desktop network change was performed.

## Reproduction

Run one opt-in test at a time, with the approved parent empty:

```sh
export BROWSEY_TEST_CLOUD_SCOPE=rclone://Onedrive/agent_test_folder
export BROWSEY_TEST_CLOUD_WRITE_APPROVED=yes
cargo test real_onedrive_working_copy_and_archive_acceptance -- --ignored --nocapture
cargo test real_onedrive_archive_tree_acceptance -- --ignored --nocapture
cargo test real_onedrive_active_fault_acceptance -- --ignored --nocapture
cargo test real_onedrive_post_preflight_destination_acceptance -- --ignored --nocapture
```

The larger/deeper run sets `BROWSEY_TEST_CLOUD_TREE_FILES=512` and
`BROWSEY_TEST_CLOUD_TREE_DEPTH=8`. Defaults are 32 files/depth one; positive
values are bounded to 1,024 files/depth 16 before any cloud write. All five tests
can be selected with `cargo test real_onedrive_ -- --ignored --nocapture
--test-threads=1` after the native opt-in below; they must not run concurrently.

Native acceptance additionally needs a production candidate at
`target/release/browsey`, Hyprland and AT-SPI Python bindings. It refuses a
symlink to the installed binary:

```sh
export BROWSEY_NATIVE_CLOUD_TEST_APPROVED=yes
cargo test real_onedrive_native_archive_acceptance -- --ignored --nocapture
```

The successful native workflow above used candidate SHA-256
`4b4bbb8039247c92dd568a97f107454e26f1a2f99e4397e3516621a319801713`.
It included the pipe capture/shared classifier changes, but predates the later
adapter-wide URL scrub and concurrent-copy guard. It is not evidence that those
later changes were installed or that the final checkout passed native acceptance.

## Still open

Broader scale and mixed-large-file budgets, dedicated-account quota/rate limits,
destination changes during an in-flight provider write, external GTK receiver, provider-web trash
restore, and approved Google Drive/Nextcloud acceptance remain open. See the
[active TODO](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md) and
[OneDrive checklist](../../cloud/checklists/onedrive-rclone-v1-manual-checklist.md).

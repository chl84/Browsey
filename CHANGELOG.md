# Changelog

## Unreleased

- Give Properties a concise accessible dialog name and focus the main menu
  when it opens. Restore its opener before opening another dialog and handle
  Escape once. Verify native keyboard/focus behavior during a real transfer.

- Preserve cut mode and all file paths when an X11 clipboard owner returns a
  URI list for the GNOME target; clear X11 clipboard without waiting on an open
  input pipe. Verify isolated two-window clipboard and private trash operations,
  and warn accurately that purging the Wastebasket cannot be undone.

- Verify native drag-label tracking across loaded lists/grids and empty space:
  stable feedback nodes, recovery from rejected file hovers, cleared feedback
  after cancellation/release and one independently verified copy per drop.

- Preserve the internal drag action when WebKit ends its DOM source before
  Tauri delivers the matching native drop. Verify native file/folder and mixed
  drags, self-drop/cancellation, two owned Browsey windows, isolated Nautilus
  copies and teardown during an active drag on a private X11 desktop.

- Define a separate approval/isolation contract for provider connection, locked
  phones, reconnect, disappearing mounts and busy/ejected media. Keep ordinary
  native-session restrictions; real lifecycle transitions remain separately scoped.

- Honour explicit cloud overwrite even when source and destination have equal
  sizes and modification times. Force requested replacement in rclone CLI and
  per-call RC transfers while preserving new-file conflict protections.

- Verify native OneDrive refresh, transfers, all four file conflict choices and
  private working-copy preparation/upload, including an equal-size changed
  original. Check preserved cut sources and truthful failure with bounded,
  candidate-only quota/rate/authentication faults.

- Verify native MTP operations and source preservation on provider name failure,
  plus decoded cold/late image thumbnails, stable ordering, explicit refresh and
  generated-file metadata in the approved mobile test folder.

- Verify native SFTP/GVFS/FUSE operations, byte progress, cancellation and stale
  paths. Permanently delete a bounded generated 32 MiB file without a local undo
  content copy, preserving it when confirmation is cancelled.

- Verify native USB behavior on the actual ext4 mount: file/folder operations,
  transfers in both local directions, Properties capabilities, real access
  denials and copying a read-only file with its supported mode preserved.

- Verify five native local Unix link cases: relative/broken link listing and
  explicit clipboard rejection, independent hard-link copy and same-inode alias
  move. Restrict test-only exceptions to exact owned paths/known aliases, capture
  private link identities for no-follow audit and retain all link fixtures.
  Complete the scoped NT4 names/data/tree/link run (90 unique native parts).

- Verify 18 native bounded-tree/listing parts across all five storage types:
  reduced device fixtures, empty/deep/wide copies, recursive search with released
  resources and exact 200-file local virtual selection/copy digests. Bound native
  test dispatch traversal by both entry count and depth.

- Release completed cloud and mixed-transfer progress listeners without losing
  the completion timer or transfer result. Verify 18 native binary copy/move
  parts across all local-hub storage routes using raw bytes and SHA-256 digests,
  including zero/one-byte files and preserved unrelated data.

- Reject unsupported Unix filename encoding explicitly in directory listing and
  recursive search, preserving the original files instead of exposing lossy
  aliases. Validate single/batch rename leaf names before mutation and explain
  empty rename drafts. Verify 20 scoped native reserved-name, encoding and
  name/path-boundary parts across all five storage types.

- Preserve significant whitespace in created and renamed filenames instead of
  silently trimming it. Verify 29 scoped native name/URI parts on all five
  storage types, including emoji/combining Unicode, literal URI characters and
  explicit quoted-name rejection on the tested MTP device.

- Verify safe native startup after interrupting an owned overwrite process:
  retain exact partial output and protected originals, keep recovery markers
  across restart, start with empty undo history and expose read-only backup
  diagnostics. Complete the scoped NT3 failure-safety run (107 native parts).

- Verify native transfer safety with concurrent owned writers: same-size edits,
  source removal/symlink swaps, late overwrite collisions and changed move
  sources after cloud upload. Preserve uncertain output and protected originals,
  and report failure rather than successful completion.

- Honour clipboard cancellation during cloud destination preparation and
  between selected roots. Make the metadata read cancellable with the transfer's
  token and keep cancellation pending until the caller acknowledges its reply.
  Verify bounded no-space, unavailable-provider and transient I/O failures with
  preserved sources, retained partial output and released progress callbacks.

- Verify native Properties permission/ownership capabilities on local disk,
  USB, network, cloud and mobile, plus real local read/write denial and copying
  of a read-only file with its supported mode preserved.

- Preserve local move sources when cancellation arrives after cloud upload or
  their file version changes during upload. Check cancellation before deleting
  a downloaded cloud source. Verify scoped native interrupted/failed moves on
  all five storage types, with source retention, truthful partial batches and
  released progress callbacks.

- Add scoped native overwrite-cancellation/failure checks: preserve original
  destination bytes, retain protected recovery backups for partial local/USB
  writes, and report uncertain rollback without claiming successful completion.

- Register mixed-transfer cancellation before asynchronous route validation,
  so early cancellation reaches the task. Verify cancellation before writes
  on all local-hub storage routes, with representative local mid-file and
  local/cloud between-file cases, truthful partial counts and released callbacks.

- Keep missing cloud transfer sizes indeterminate instead of reporting a
  fictional one-byte total. Show zero/unknown progress and actual completed
  file bytes through finalization. Verify visible progress and activity cleanup
  in 27 scoped native parts across local disk, USB, network, cloud and mobile;
  large-file throughput and universal mid-file callback cadence remain outside
  this small-fixture acceptance.

- Preserve empty cloud folders and nested empty directories during copy and
  overwrite-move merges, removing source directories only after a successful
  move. Allow cloud-to-phone transfers when MTP cannot set file timestamps,
  without changing timestamp handling for other destinations.

- Add Skip to transfer conflict choices alongside Overwrite, Auto-rename and
  Cancel. Keep skipped and failed cut sources in the clipboard, and preserve a
  newer clipboard selection when an older transfer finishes. Explicitly refuse
  cloud or mixed file/directory overwrite when safe replacement is unsupported.

- Reject same-parent cuts and unsafe self, descendant or source-ancestor targets,
  including case-insensitive cloud aliases. Keep valid same-parent copies under
  distinct names, and bound cloud/mixed unique-name attempts to 50 candidates
  without replaying unknown write failures.

- Report completed, skipped, failed and unattempted roots after a partial paste.
  Preserve local rollback behavior and show unknown counts when rollback cannot
  be verified. Keep the original transfer error visible through activity cleanup
  and subsequent refresh failures, and observe listing errors before claiming
  refresh success.

- Complete scoped NT2 native copy/move acceptance within and between local disk,
  USB, network, cloud and mobile, including conflict choices and unsafe targets.
  Independently verify both generated trees and retained sources. Batch-failure
  coverage uses representative local/USB/cloud cases and bounded candidate-only
  faults; it does not certify provider outages or network/mobile failure modes.
  See the [NT2 verification record](docs/operations/linux-release/runs/2026-10-06-native-transfers.md)
  for accepted scopes and retained failed reports.

- Convert cloud search path errors through a typed adapter, preserving invalid
  path diagnostics while satisfying the blocking Semgrep error-boundary checks.

- Search recursively inside cloud folders using the existing rclone provider,
  streaming matching names and metadata from subfolders without downloading
  file contents. Preserve provider errors and cancellation, and reject missing
  search start directories instead of falling back to the home directory.
  Clear old matches when a search draft changes without Enter, keeping the
  current folder and requiring submission before displaying new results.
  Add scoped native sort/filter/hidden-file/search cases for all five storage types.

- Fetch a fresh cloud folder listing when explicitly refreshing through F5 or
  the main menu, including recently added OneDrive files. Prevent F5's WebView
  default before waiting for I/O. Add scoped native list/grid navigation coverage
  across local, USB, network, cloud and mobile test folders.

- Recover the Linux GVFS FUSE bridge using distribution-specific executable
  locations and actual session mount readiness, rather than a permanently cached
  process check. Retry missing GVFS paths and verify network/phone mount paths
  before reporting a successful connection.

- Add an opt-in Linux native acceptance harness using WebDriver and the existing
  AT-SPI helper, with shared small-file cases across explicitly approved storage
  targets. Isolate the candidate's profile and enforce owned-path IPC limits;
  exclude credentials, local approvals and runtime artifacts from Git. Device
  lifecycle and large-transfer acceptance remain separate from the small-fixture
  native runs.

- Keep the drag label mounted when moving between file rows/cards with a null
  `relatedTarget`; these are not webview exits. Publish listing drag inputs only
  when target, source count or dragging state changes, not for pointer position
  updates, and skip redundant target notifications. Retain an unchanged valid
  target/highlight on background movement and repeated native hovers instead of
  clearing/reapplying it. Repeated hovers no longer rebuild Explorer prop bags
  or refresh thumbnail bindings; drop safety is still checked on each hover.

- Keep internal drags active on zero-button pointer motion, preserving DOM label
  position updates and explicit copy/move actions. Recover abandoned state on
  fresh pointer presses/releases, not ordinary movement.

- Hide drag feedback when leaving the window or delivering a drop, independently
  of the retained source selection. Recover from a missing DOM `dragend` on normal
  pointer presses/releases or a different incoming native offer after leaving.
  Keep returning self-drops and their explicit copy/move action intact, and do
  not interrupt or duplicate an accepted transfer while it finishes.

- Notify the desktop after a verified user-local installation, preferring
  Omarchy's notification command. Keep notifications optional and time-bounded;
  failed installs and dry runs never send a success notification.

- Use native GIO byte progress for GVFS/network copies in both directions, with
  cancellable I/O independent of progress callbacks. Never automatically retry
  a failed or cancelled GIO copy through a different writer. Aggregate progress
  across files and folders, count file contents rather than directory metadata,
  and retain uncertain outputs without deleting the source. Release paste
  listeners after completion; only the entire paste finishes its progress task.

- Share keyboard and context-menu clipboard/deletion actions, including progress,
  cancellation, network confirmation and partial-result refresh. Keep successful
  mutations distinct from subsequent refresh failures and never retry deletion.
- Enforce concrete ExplorerShell prop, modal-state and callback contracts from
  assembly through rendering, without untyped prop-bag fallbacks.
- Extract USB-format modal orchestration with stale-response/progress guards.
  Preserve formatting outcomes and repair watches even when drive refresh fails.
- Move Explorer shortcut composition and file-action orchestration into focused
  controllers, retaining the existing keyboard router and shared mutation policy.
  Release cloud-open progress listeners after both success and failure.
- Reuse shared error normalization in Properties, retaining nested and
  JSON-encoded IPC error codes and permission-specific recovery messages.
  Preserve existing Error instances, stacks and diagnostic metadata.
- Finish failed network-delete progress cleanup before showing confirmation,
  so a quick confirmation cannot lose the next operation's progress UI.

- Delete Linux network/GVFS files directly through GIO instead of downloading
  them into local undo backups. Use the mounted server's trash when supported;
  otherwise require explicit permanent-delete confirmation even when ordinary
  confirmation is disabled. Preserve local undo in mixed batches, item progress,
  cancellation and partial-outcome reporting. Never silently retry a failed
  trash operation as permanent deletion.

- Show each saved SFTP/FTP connection only once in Network, merging server-root,
  home-folder and existing GVFS mount aliases while preserving the saved target.
  Keep separate accounts, ports and SMB/AFP shares or NFS/WebDAV scopes distinct.
  Forget Connection removes saved folder aliases for that connection together.

- Use GTK/GVFS authentication dialogs for Linux network mounts, including SFTP
  username/password, optional system-keyring storage and host-key questions.
  Bound and cancel pending connections, report the real backend error, and open
  the exact server/account/folder rather than an unrelated protocol mount.
  Remember successful server addresses (never passwords) in Network across
  restarts, deduplicate mounted/discovered entries, and add Forget Connection
  without disconnecting the server or deleting keyring credentials.

- Show partition names one word per line, truncating overlong words with an
  ellipsis instead of breaking them across lines. Keep full labels available to
  assistive technology and in the existing tooltip.

- Fix a stuck Loading indicator when a silent directory refresh supersedes a
  foreground load: the replacement inherits responsibility for clearing it on
  success or failure, while stale replies cannot clear newer loading state.
  Local watcher refreshes no longer interrupt user navigation or active search,
  preventing the stuck state from blocking subsequent drag-and-drop transfers.

- Keep GVFS/MTP grid order stable during automatic directory refresh and preserve
  known metadata while placeholders are refreshed, so camera thumbnails do not
  shuffle or restart just because lazy metadata arrives. Coalesce polling and
  watcher refreshes, share one metadata scan per directory, and ignore stale
  directory replies after navigation or explicit sorting. Manual refresh still
  adopts sorted snapshots; GVFS sort changes use already-loaded entries.

- Add NTFS USB formatting via UDisks and `mkntfs`, with 128-character ASCII volume
  names. Show missing filesystem tools as disabled choices in the shared format
  modal, validate names when switching formats, and check daemon support before
  unmounting or erasing the drive. Ask UDisks to match the GPT partition type to
  the filesystem, including Windows-compatible formats. Existing whole-drive
  warnings, job progress,
  mount handling, and no-automatic-retry protections remain in place.

- Remove the Copy drive path button from volume Properties while retaining the
  mount point/device path.

- Respect read-only mounts and NTFS/network permission capabilities in Properties.
  Keep supported POSIX edits available, disable synthetic or unverified controls
  with short explanations, and verify permission/ownership changes and rollback
  results rather than trusting syscall success. Never request local privilege
  elevation for a batch containing network targets.

- Disable unsupported Read, Execute and ownership changes on Linux exFAT/FAT
  mounts, with a short explanation. Preserve supported file write-protection
  toggles, respecting mount masks and verifying the actual result.

- Open Properties for fixed volumes as well as removable drives from the sidebar
  context menu. Keep Basic compact with a theme-aware, shared usage meter and
  fresh total/used/free filesystem statistics, without scanning contents or
  automatically mounting devices.

- Show small decimal-GB capacity labels beside local partitions and USB volumes
  in the sidebar, reusing Linux device metadata without probing remote mounts.
  Show shared root-volume capacity only at `/`, retaining labels for separate
  partitions even when their capacities match.

- Add a compact, red-outlined Reset button beside the active column-filter
  indicator in grid view, clearing column filters without changing search or sort.

- Distinguish resolved historical verification failures from unresolved failures
  in maintenance reports; require evidence of a later successful rerun before
  recording completed work, while preserving skipped checks and remaining risks.

- Keep Settings and Properties open when Escape dismisses a ComboBox menu,
  restore focus to its trigger, and reveal highlighted options within long
  lists when opening or navigating with the keyboard.

- Add a manual T3 maintenance-report follow-up action with report-identity checks,
  private per-finding handling history, verified command evidence and retained
  interrupted attempts. Keep weekly reviews read-only and implementation,
  commits, installation and publication separate.

- Let Escape dismiss shared tooltips and cancel pending tooltip display without
  moving focus or consuming the key used by dialogs and other controls.

- Improve Ctrl-wheel zoom responsiveness: retain rapid notches, coalesce target
  sizes per animation frame, and publish the final anchored grid window without
  intermediate layout passes. Debounce thumbnail resolution upgrades while
  keeping existing previews and useful in-flight work; preserve selection,
  modal blocking, compact density and list/grid transitions.

- Translate remaining Norwegian code and SVG comments and the T3 installation-action
  label to English; preserve intentional Unicode fixtures and historical filenames.

- Consolidate import/naming policies under docs/architecture, update project
  layout guidance and current links, and remove obsolete COPR automation while
  preserving the separate manual RPM packaging path and historical records.

- Audit and correct the documentation site: separate runtime/source requirements,
  update archive/cloud/recovery guidance, add usable reference links and concise
  release highlights, and improve search, keyboard navigation and responsive
  menus. Add route/content/link regression tests and non-deploying PR checks.

- Simplify the GitHub README into a product overview with linked installation,
  user and developer guides. Separate runtime/build requirements, use the pinned
  npm Tauri CLI in source instructions, clarify Windows build limitations and
  retain cloud/recovery safety boundaries. Keep release bumps compatible with
  normal Markdown header spacing and check relocated usage details in docs.

## v1.0.4 — 2026-10-03

- Restore a fixed 5 px gap between list-header sorting and filter buttons, retaining complete-header minimum widths, separate resize targets and 8/6 px SIZE content padding.
- Add a 3D model file icon using the generic document silhouette and a shaded cube. Recognize common model/scene/CAD extensions (including Blender, STL, OBJ, FBX, glTF, USD and STEP), as well as model MIME types, without renumbering existing icon IDs.

- Right-align file sizes and folder item counts with 8 px right padding in Cozy and 6 px in Compact. Measure complete list headers to enforce minimum widths during dragging, saved-width restoration and density/font changes, keeping filter and resize targets separate.


- Keep the SIZE header left-aligned and align stars to the left edge of their own column without reducing the star click target.

- Add "Empty Wastebasket…" to the Wastebasket sidebar context menu, with a permanent-deletion warning in the shared confirmation modal, Cancel focused by default and controls locked while working. Empty the native system-trash catalog off the UI thread, preserve cloud trash, and refresh/report partial failures without automatic retries.

- Reuse the compact Properties action-button sizing for "Copy drive path" and "Apply ownership", keeping equal typography, padding and height in Cozy and Compact layouts.

- Keep list-header filter buttons separate from column resize handles, including SIZE and narrow columns. Remove overlapping alignment offsets while preserving sorting, filtering and resizing.

- Simplify Settings stored-data controls: remove drag-and-drop prose, group named cache/list cleanup actions, and show a concise recovery-backup overview with allocation, paths and manual recovery guidance collapsed by default. Preserve read-only inspection, safety warnings and existing cleanup confirmations.

- Do not retry failed RC jobs or ambiguous responses after submitting a cloud write. Preserve completed-job provider codes, keep pre-submission fallback separate, bound RC response buffers and redact entire signed OData URLs (including apostrophes) and JSON secret fields in failure feedback/debug output.

- Drain rclone stdout/stderr concurrently instead of waiting on pipe-blocked children; bound capture memory and report unknown completion rather than accepting truncated output. Preserve the end of lengthy failure diagnostics, redact signed provider URLs and reuse common typed cloud error classification for mixed transfers, including network loss and timeout. Add ownership-guarded OneDrive archive-tree and active cancellation/transport-fault acceptance checks.

- Protect new-object cloud copies against destinations appearing after Browsey's preflight. Use rclone's existing-object skip/no-transfer guard for single-file CLI copies and per-call skip plus completed-transfer verification for RC progress uploads; never report a skipped copy as successful. Preserve explicit overwrite/move behavior and document the remaining provider/in-flight race boundary; validate competing/source bytes on real OneDrive.

- Add opt-in, isolated 10k/100k listing/search, mixed-thumbnail, recovery and controlled-cancellation workloads with structured measurements and browser/native helpers. Avoid redundant local metadata caching and bound the existing network metadata cache to 10,000 entries without changing freshness, ownership or transfer semantics. Expose list/grid views as labelled button collections rather than malformed ARIA tables, restoring native WebKit file accessibility.

- Measure representative undo-backup allocation and history retention with opt-in disposable workloads. Show filesystem-reported allocation separately from file-content size in the existing Settings diagnostics; document a conservative storage policy without introducing automatic recovery deletion or a hard quota.

- Fix cloud archive extraction and other mixed copies sending `--create-empty-src-dirs` to unsupported `rclone copyto`. Select `copyto` for files and `copy --create-empty-src-dirs` for directories, explicitly preserve entirely empty directory roots as well as nested empty folders; tighten fake-rclone argument validation and cover encrypted staged ZIP round trips and the real local-only rclone CLI contract.

- Add a T3 Code project action and Linux x86_64 user-local installer that builds the current checkout with Tauri's production frontend, stages runtime resources, preserves an installation backup and desktop integration, and leaves running operations alone. Regression checks cover build/runtime failures, rollback, installation locking and Rust toolchain selection.

- Expand cloud folders with private persistent working copies/recovery access, new files, Open With, explicit save-as-new uploads with source-change checks, provider-aware OneDrive/Google Drive trash, staged archive compression/extraction and passwords, preflighted advanced rename, and prepared copy-only external drag. Reuse transfer/archive/cancellation/activity/capability layers; keep originals and staging on failure, protect edits from preview-cache clearing, preserve empty copied directories and refresh after partial outcomes. No automatic sync, atomic cloud overwrite, cloud undo or Nextcloud trash API is claimed. Real-provider acceptance is tracked separately and remains open until approved disposable remotes are tested.

- Keep Semgrep typed-error checks clean with explicit I/O, runtime-task and thumbnail task-error conversions, preserving existing IPC codes instead of reclassifying diagnostic text. Share local/CI scan options, make blocking findings and invalid configuration fail, and regression-test positive/negative fixtures, exclusions and exit codes without weakening rules or adding exceptions.

- Correct the shared Settings RAR capability note and its filter text: compressed/password-protected RAR extraction is supported; RAR archive creation is not.

- Show failed archive names and reasons in partial batch extraction, with bounded details and correct singular/plural counts. Refresh listings after extraction errors or cancellation; report refresh failures separately with F5 guidance instead of mislabelling completed extraction as failed. Do not retry file operations automatically.

- Separate daily-driver engineering TODOs from outstanding acceptance, optional product decisions and release rules. Archive 33 already-verified coverage/safety increments with their evidence, preserve 29 unverified checks in a dedicated validation checklist and keep five engineering follow-ups active. Do not mark moved tests passed or imply missing functionality from unchecked acceptance.

- Compare local manual-copy outputs with BLAKE3 digests of the written stream before recording completion or deleting fallback move sources. Share bounded open-handle readback between clipboard and undo engines, recheck versions around verification, retain uncertain output on mismatches/read failures and honor clipboard cancellation between readback chunks. Reuse the existing dependency, keep verification reads out of transfer-byte totals and document the extra target read pass, measured warm-cache cost and remaining post-check races. GIO/cloud writers and native same-filesystem rename are unchanged.

- Protect original destinations before moving them into clipboard overwrite backups, closing the process-interruption window before failure rollback. Carry protection through the whole paste/merge and reuse it during rollback and undo/redo; only clear it after whole-operation success. Stop before moving originals when protection fails, preserve uncertain backup candidates, and verify killed copy/move/merge processes, marker faults and history lifecycle with disposable fixtures. Ordinary delete/trash protection scope is unchanged; this is not persistent undo or power-loss durability.

- Retain uncertain local file-copy outputs after read/write/writeback failure or cancellation instead of unlinking potentially edited files. Check cancellation before creating the target, report partial output for inspection, and protect original overwrite backups before failure rollback. Keep blocked backups across startup cleanup, clear markers only after successful rollback, and refuse rollback if protection cannot be written. Cover in-place edits, copy/move overwrite failures, actual file-size limits and separate-process cleanup; completed-output rollback and active-write success limitations remain distinct.

- Revalidate completed fallback-copy receipts before deleting move sources, and reuse recorded per-entry removal instead of recursive source-tree deletion. Preserve late source additions/edits and changed destination contents; honor cancellation during source removal and report partial outcomes without automatic retry. Refuse destructive fallback completion for writers without ownership receipts, including GIO-owned copies, retaining sources and copied output for inspection.

- Validate opened source versions and copied lengths after local file streaming, and check pre-sync output versions after writeback in both copy engines. Reject changed inputs and in-place target edits without adopting them into receipts or deleting sources; preserve uncertain finalized outputs and edited targets during error cleanup. Add deterministic rewrite/truncate/append and finalization-edit regressions while keeping final-check and active-writer limitations explicit.

- Show read-only undo/recovery backup diagnostics under Settings > Stored data, with bounded metadata scans, explicit incomplete measurements, a copyable storage path and manual recovery guidance. Preserve backups, locks and markers; distinguish file-content size from disk usage and 50-action history from a storage quota. Reuse shared size formatting and cover refresh failures, pending requests, keyboard access and narrow-window layout.

- Preserve copied bytes before undo removes targets; restore redo and failed mixed-batch compensation from verified private backups instead of changed/missing original sources. Keep recovery markers through the whole operation and preserve marked sessions during startup cleanup, including after process interruption. Move history work off the event loop, reconcile listings after errors, distinguish refresh failures and suppress repeated undo/redo requests; cover backup/restore faults, source edits, native filenames, cross-filesystem copies and recovery-session cleanup.

- Guard copy undo and clipboard batch rollback with receipts of created directory identities and completed open-writer versions. Preserve edited/replaced outputs, reject unverifiable GIO-owned copies, and remove only recorded files and empty directories instead of whole trees. Preflight pure-copy batches before deleting any member; cover normal undo/redo, merge overwrite backups, cancellation and deterministic post-scan mutations. Document remaining mixed-batch recovery and filesystem-race limits.

- Replace recursive failed-copy cleanup with individual output ownership/version tracking, preserving untracked, replaced and edited destination files. Revalidate regular and nested source versions before fallback move deletion. Reconcile local paste failures without automatic retry, retain original errors, and distinguish completed transfers from listing-refresh failures; cover these outcomes with filesystem and mocked UI regressions.

- Verify local copy/move and undo fallback with deterministic partial-write, disk-full, read/write disappearance, writeback and cancellation faults. Reuse stable archive-output identities for copies; retain sources when the written target is missing or replaced, and preserve competing files during error cleanup. Report retained undo partial outputs explicitly and track verified safety increments separately from platform acceptance.

- Harden local moves and undo fallback: use atomic no-replace rename, honor cancellation before moving, retain the completed destination after partial source deletion fails, sync fallback copies before source removal, and surface failed cancellation rollback. Reuse a regular-file input guard across copying/undo/compression so FIFOs and replacement symlinks cannot hang or redirect reads. Add deterministic race/failure, filename, undo-history, and killed-session cleanup regressions.

- Archive the completed Linux 1.0 readiness track, update its references, and distinguish historical audits/RC results from current release validation while retaining reusable safety gates.

- Hide custom minimize/maximize buttons on Hyprland, retaining menu/close and normal controls on other desktops. Respect Omarchy's window-management policy without changing compositor settings; handle window-action failures and cover desktop detection and titlebar rendering with regression tests.

## v1.0.3 — 2026-09-30

- Replace the newly yanked yoke-derive 0.8.3 with compatible 0.8.4 before publication, retaining native/desktop pins and the existing manifest constraints. The upstream patch restores minimum-Rust-version compatibility; registry audits retain only the two documented unmaintained build-time notices.

- Add a preview-first release bump helper coordinating app version metadata, current README/docs references, changelog and new release notes. Require a clean working tree for writes, reject existing notes/tags and failed origin checks, preserve dependency pins and historical releases, and optionally run strict maintenance/docs gates. Keep release validation checklists unchecked and commit/push/tag/publication/installation explicit; cover planning, safe application, failures and preservation with isolated regression tests.

- Honor a directory path or local `file://` URI passed at launch, fixing default-file-manager folder launches such as T3 Code's Open action. Resolve relative paths from the launch working directory, retain the saved start folder when no argument is supplied, and surface invalid arguments without silently opening Home. Add backend parsing and frontend startup/recovery regression coverage.

- Make dependency security checks blocking, add weekly grouped Dependabot proposals without auto-merge, and verify coordinated Tauri/native pins in CI. Keep the upgrade helper within existing manifest constraints and document the two remaining indirect build-macro maintenance warnings and platform-validation limits.

- Refresh the SQLite persistence, disk-information and async runtime dependencies to rusqlite 0.40.2, sysinfo 0.39.6 and Tokio 1.53.1, align Zstandard at 0.14, and update compatible Rust lockfile dependencies while retaining explicit native/desktop ABI pins.

- Update ZIP to 8.6.0 and 7z to 0.23.0, including hardened header parsing and refreshed codecs/cryptography. Recheck encrypted archive and extraction-safety regressions and round-trip non-solid archives with all nine enabled 7z codecs.

- Update image decoding to image 0.25.10 and SVG rendering to resvg 0.48.1, replacing unmaintained font parsers with harfrust/skrifa. Restore arithmetic-filter thumbnails after the upstream fix and test oversized filters, nested transforms, viewBox sizing and real font rendering.

- Refresh the statically linked native UnRAR decoder to stable 7.23 while preserving Browsey's streaming/password/cancellation adapter. Match the native packed DLL structures, add runtime size/offset/source-version checks, and record source hashes and full packaged decoder/binding licenses.

- Coordinate Tauri core/API/CLI 2.12.0 with tauri-build 2.7.0, update the plist/XML parser security fixes, and backport the GLib mutable out-pointer fix without breaking GTK3/GIO's ABI. Verify vendored-source provenance and native drag/teardown on an isolated virtual screen; cover GLib iteration in optimized builds.

- Refresh frontend and docs-site Svelte/Vite/tool dependencies within their current major versions, resolve npm security advisories in both lockfiles, and bind the desktop development server to loopback instead of all network interfaces.

- Add weekly and dependency-change security checks for Cargo and both npm projects. Update TAR to 0.4.46 and compatible indirect Rust security fixes; cover PAX size overrides across GNU long-name headers with real extraction regressions.

- Update bundled Linux and Windows PDFium to `156.0.8076.0` and `pdfium-render` to `0.9.4`, with an explicitly pinned API profile, verified release-asset/binary checksums, refreshed headers/licenses, and immutable third-party provenance. Share one process-lifetime PDFium runtime between thumbnails and metadata; handle fallible image conversion and keep extreme-aspect thumbnails nonzero. Add real PDF regressions for rotations, fonts, metadata, malformed/encrypted inputs, repeated calls, and concurrent requests.
- Fix the mount refresh interval slider collapsing to zero width in Settings. Keep its value and unit together, place helper text below the control, and add accessible labeling and pointer/keyboard layout regression coverage.
- Add optional AES-256 password protection when creating ZIP archives, with password confirmation and a visible-filenames warning. Extract password-protected ZIP (AES and legacy ZipCrypto), 7z, and RAR archives, including encrypted 7z/RAR headers. Prompt per archive, support wrong-password retries and cancellation, and retry only password failures in a batch. Keep passwords out of settings/logs, clear dialog inputs, zeroize owned backend input buffers, and add real encrypted-archive and browser regression tests.
- Harden archive operations: report final buffered-write failures, preserve unrelated/replaced files during extraction rollback, create private outputs and restore safe Unix permission bits, and correctly extract a lone empty 7z file. Reject FIFOs/devices/sockets and unrepresentable ZIP filenames instead of hanging or silently renaming them. Clean up compression outputs on preflight failure, register cancellation before scanning, and bound/cancel TAR preflight decoding. Add archive-content, permission, rollback, cancellation, and error-injection regression tests.
- Keep Linux drag portal tokens valid across repeated destination reads, fixing Nautilus drops rejected as `Invalid transfer`. Reuse one token per drag and explicitly stop it on completion, cancellation, or teardown. Add portal lifetime and shared-session Nautilus regression coverage.
- Use GIO for Linux default-app opening as well as default-app selection, fixing Python files whose MIME type differs between GIO and xdg-open. Return handler lookup/startup errors to the UI, and perform file opening off the UI thread. Add isolated real-launch regression tests for Python files and failed launches.
- Add an unchecked Set as default checkbox to the left of Cancel in Linux Open With. When checked, Open saves the selected application as the desktop default for the displayed MIME type and then opens the file. Validate installed desktop IDs and recheck the file type before saving; keep save failures and partial-success launch failures visible for retry. Directories and unknown file types cannot change defaults through this action.
- Export local files to other applications with ordinary drag, without Alt. Keep WebKit's original drag session and deliver a correctly escaped native URI list through a small GTK bridge, including multiple files and special characters. The receiving application handles copy/move; Browsey never deletes sources on drag completion.
- Fix the Linux window-close crash after file drag by removing completion IPC channels from GTK callbacks. Retire the separate drag plugin/backend and test real WebKit export followed by Tauri window teardown.
- Resolve native drops from the actual pointer position (including display scaling), with shared highlighting for folders, breadcrumbs, bookmarks, mounted drives, and list/grid background. Reject ambiguous backgrounds and drops while dialogs or navigation are active.
- Respect explicit local drag-start actions (Ctrl/Meta locks copy, Shift locks move) across internal and external destinations. Otherwise use live modifiers for internal drops and local filesystem-aware defaults. Copy cloud transfers and incoming external files by default; retire stale previews and pending listeners when a drag ends.
- Add edge autoscroll and delayed folder opening during drag, with download-first guidance for exporting cloud files. Document these gestures in Settings > Shortcuts.
- Isolate drag-and-drop sources from the clipboard so native drops cannot copy or move a previously selected cloud file.
- Keep source paths, destination, and copy/move mode fixed through conflict preview and confirmation for local, cloud, and mixed transfers. Reject overlapping requests and repeated confirmation, and preserve newer clipboard selections when an older move finishes.

## v1.0.2 — 2026-09-25

- Discover connected MTP phones through GIO without first opening another file manager; mount on demand and show phone-specific properties and unlock guidance.
- Grant the formatting user ownership of new ext4/btrfs USB filesystems, and expose Properties for removable drives. Formatting does not change ownership of existing volumes or phone storage.
- Support USB formatting to exFAT, FAT32, ext4, and btrfs when the matching system tools are installed; inspect the target and require destructive-action confirmation.
- Prioritize visible thumbnails, cancel stale work on scrolling/navigation, reuse cached results, and avoid opening original contents on disk-cache hits after path/metadata validation.
- Bound thumbnail workers, improve JPEG decoding and cancellation, and evict disk-cache entries by byte budget and recent use rather than a fixed 2,000-entry cap.
- Add Ctrl + mouse-wheel zoom from list into five grid sizes (64, 96, 128, 160, and 192 CSS pixels), with sharper thumbnails and a retained view anchor. Zoom is window-local.
- Increase grid gaps to 8 px in Cozy and 6 px in Compact, and reduce system-theme lasso opacity to 16% without changing selected-file highlights.
- Offer system/Omarchy colors in Settings with light/dark fallback.
- Show delete/trash progress in items instead of incorrectly labelling item counts as bytes.
- Replace the previous RAR reader with a streaming UnRAR adapter, covering compressed RAR4/RAR5 and complete multi-volume archives while retaining extraction limits, cancellation, and path-safety checks. Password-protected archives report an explicit error.
- Wait for the actual UDisks formatting reply instead of the CLI's default timeout; treat lost replies as unknown status and never automatically repeat an erase.
- Check device-scoped UDisks jobs before formatting or retrying, and reject overlapping Browsey formatting requests.
- Reuse a shared progress bar in the topbar and USB format dialog, showing real per-job percentages only when available and indeterminate progress otherwise.
- Add a private D-Bus regression test with a 26-second simulated format, plus UI tests for progress and uncertain completion.

- Keep compression failures visible and allow retry; prevent duplicate submissions from closing the dialog.
- Keep Open With selection inside the filtered app list and correct its empty-state text.
- Preserve directory monitoring on rejected USB formatting requests and reattach watches after formatting.
- List unmounted removable USB partitions with a Mount and open action; refresh on UDisks events with polling as a fallback.
- Keep formatting errors in the dialog with copyable details and require fresh inspection before retrying.
- Add context-menu keyboard focus, arrow/Home/End navigation, focus restoration, accessible dialog titles, and longer-lived status messages.
- Add regression tests for compression, filtered application selection, USB discovery and failures, keyboard menus, and hotplug refresh.

## v1.0.1 — 2026-09-10
- Preserve files created in a source directory during merge-move operations; abort and roll back instead of recursively deleting the remaining source contents.
- Reject symbolic links before resolving local-to-cloud source paths, including dangling links and single-entry transfers.
- Keep undo backups in locked, per-process sessions. Startup cleanup removes only abandoned sessions, leaving live sessions and legacy backups intact.
- Remove partial file copies after read, write, permission, or flush failures so overwrite rollback can restore the original destination.
- Preserve private and executable file permissions and directory permissions during local copies and cross-filesystem undo copies.
- Add regression coverage for concurrent writes, interrupted overwrites, cloud symlinks, permissions, and cleanup across processes.
- Repair the frontend theme import boundary and the ambiguous USB smoke-test selector.

## v1.0.0 — 2026-03-07
- Browsey Linux 1.0:
  - Linux 1.0 release signoff is now based on a completed stabilization track across core file workflows, Linux-specific runtime behavior, packaging/install validation, observability hardening, and release gating.
  - Core local trust-sensitive flows (copy/move, rename, trash/delete, compress/extract, search, properties/permissions, open-with, and settings persistence) were hardened and validated with expanded automated coverage, Linux smoke runs, and real-use verification.
  - Supported cloud providers are now part of the Linux 1.0 claim: OneDrive, Google Drive, and Nextcloud via `rclone`, with explicit provider scope, setup diagnostics, and controlled provider acceptance checklists.
  - Linux release operations now include explicit pre-release gates, a bounded stabilization window, provider-specific cloud QA appendices, and a release-candidate log tied to the existing core-operations release-blocking policy.
- Bundled dependencies and resources:
  - Bundled PDFium was updated to `147.0.7713.0` for both Linux (`resources/pdfium-linux-x64`) and Windows (`resources/pdfium-win-x64`), including refreshed binaries, headers, and license files.

## v0.4.6 — 2026-03-04
- Settings completion and polish:
  - `High contrast`, `Scrollbar width`, `Rclone path`, and `Log level` are now fully wired through persistence, runtime state, and app behavior.
  - `Restore defaults` now resets persisted settings and shortcut bindings back to project defaults, and requires explicit confirmation before applying changes.
  - `Hardware acceleration` now defaults to `off`, with a mild caution in Settings.
  - `Clear cloud file cache` was added under Settings > Data for manually removing cached local copies of opened cloud files.
- Shortcut and navigation updates:
  - Added topbar back/forward buttons to the left of the address field.
  - Added `Refresh` as a remappable shortcut with default `F5`.
  - Changed the default `Rename` shortcut to `Ctrl+R`.
  - Shortcut defaults and backend keymap metadata are now aligned so Settings reflects the real command set.
- Sidebar and text field updates:
  - Bookmark filtering is now scoped to the Bookmarks section only.
  - The bookmark filter field is hidden by default and toggled from a search button in the Bookmarks header.
  - Introduced a shared text field component for settings/sidebar inputs.
  - `Esc` now blurs focused text-entry fields before propagating to broader modal/page escape behavior.
- Scrolling and dropdown behavior:
  - Wheel-scrolling behavior is now consistent across explorer list/grid views and modals.
  - Shared combobox dropdowns now choose upward/downward opening dynamically based on available space, fixing bottom-of-modal clipping issues such as the `Log level` picker.
- Cloud delete/mkdir consistency hardening:
  - Cloud delete fallback behavior now fails closed when deletion cannot be verified: if type is unknown and both file-delete and dir-delete return `not_found`, the user now gets an explicit error instead of silent success.
  - Cloud delete policy lookup now fails closed when provider policy cannot be resolved, with regression coverage for the policy-lookup path.
  - Provider-specific delete policy is now stricter for common ghost/trash conflicts: OneDrive uses `--onedrive-hard-delete`, and Google Drive uses `--drive-use-trash=false`.
  - Cloud `mkdir` consistency was hardened by using CLI `lsjson --stat` probing and destination-exists retry/backoff only when the probe confirms the target is absent.
  - Fake-rclone parallel test reliability was improved by handling transient `ETXTBSY` (`Text file busy`) during process spawn.
- Cloud thumbnails (Grid) and cache hardening:
  - Added an opt-in `Cloud thumbs` setting for Grid thumbnails on `rclone://` entries.
  - Cloud thumbnails in v1 are limited to `image + pdf + svg`; cloud video thumbnails are intentionally blocked.
  - Added cloud thumbnail prechecks for extension allowlist, known size, and size limit (`<= 50 MB`) before materialization.
  - Cloud thumbnail generation now checks thumbnail-cache hits before cloud materialization, reducing repeated cloud downloads when navigating between folders.
  - Cloud-open cache pruning was hardened to avoid evicting managed cloud files prematurely when metadata is still fresh.
  - Cloud-thumbnail backend failures now use explicit typed-error mapping for key cloud error paths instead of relying on message-pattern fallback.
- Backend typed-error hardening and CI quality gates:
  - Transfer/statusbar/entry-metadata error paths were further migrated from string roundtrips to typed error mappings.
  - Legacy `impl From<...> for String` seams were removed in remaining advisory-hit areas (`metadata`, `watcher`), reducing accidental code-loss in error propagation.
  - Rust quality workflow now runs Semgrep typed-error checks with dual mode: advisory over `src/**` and blocking for commands-first scope (`src/commands/**`).
  - Backend hardening guard and backend test scripts were expanded to cover more backend modules and tuned to reduce false positives.
  - Hardening rollout docs were finalized and archived (`docs/todo-archive/`), with English-normalized checklist content and explicit exception policy docs.

## v0.4.5 — 2026-02-26
- Added rclone-backed cloud file support (Linux-first) with direct `rclone://...` paths and Network-view discovery for supported remotes (OneDrive primary target in v1, plus Google Drive/Nextcloud provider groundwork).
- Added core cloud file operations via rclone (`list`, `mkdir`, `copy`, `move/rename`, `delete`) with provider-aware conflict preview, overwrite/auto-rename handling, and capability-driven UI restrictions for unsupported cloud actions.
- Added mixed local-disk <-> cloud copy/move support (files and folders) for clipboard and in-app drag/drop flows, including conflict preview integration, rename-on-conflict retries, provider-aware error mapping, and refresh soft-fail behavior.
- Cloud files can now be opened directly from `rclone://...` paths through a managed local cache, including `Enter`/double-click behavior for supported file entries.
- Cloud file open and mixed cloud/local file transfers now report real byte progress when rclone rc progress data is available, with aggregated batch progress for multi-file uploads/downloads and richer byte detail in the activity pill.
- Mixed local-to-cloud writes now invalidate cloud listing cache correctly, so successful uploads appear after refresh without stale cached listings hiding the result.
- Recent view now prunes dead entries automatically and bounds slow network/GVFS metadata probes, reducing cases where an empty or mostly-stale Recent view opens slowly.
- Explorer drag/drop now supports dropping entries onto bookmark targets in the sidebar, using the same copy/move routing and conflict behavior as breadcrumb drops.
- Successful cloud/rclone operations now log at `debug` instead of `info` in release builds, reducing log noise in normal use.
- Cloud UX/performance improvements: background refresh for cloud write operations, refresh coalescing, reduced conflict-preview metadata calls, cloud remote/listing caches with invalidation, bounded per-remote concurrency, and retry/backoff for transient metadata/listing failures.
- Cloud routing hardening: `rclone://` paths no longer enter local FS/undo/GVFS paths, cloud sorting avoids remote reloads on column-sort clicks, and breadcrumbs/direct navigation now handle `rclone://` paths correctly.
- Added cloud-specific UX polish: indeterminate activity indicator for operations without meaningful byte progress, session-only manual-refresh hint for cloud folders, and corrected activity labels (`Copying` vs `Moving`) across paste/drag flows.
- OneDrive/GVFS backend support was removed in favor of rclone-backed cloud integration, and generic Linux GIO/GVFS mount helpers were renamed/refactored (`gvfs.rs` -> `gio_mounts.rs`).
- Added cloud/rclone observability and diagnostics (timing logs, scrubbed command failure logs, perf summary helper script, fake-rclone test shim, and expanded backend/frontend test coverage for cloud and mixed-transfer flows).
- Frontend architecture cleanup (no intended behavior change): explorer modules were reorganized into explicit domains (`context`, `navigation`, `file-ops`, `selection`, `ui-shell`, `state`) and old wrapper paths were removed.
- Explorer factory naming was standardized from `use*.ts` to `create*.ts` where files exported factory APIs, reducing naming ambiguity across hooks/helpers.
- Explorer state internals were split into focused slices/stores while preserving the public `createExplorerState` API to avoid integration breakage.
- Settings UI internals were modularized: `SettingsModal` is now split into tab/section components with a dedicated view-model hook.
- Frontend boundaries are now enforced with feature barrels and ESLint restrictions for cross-feature deep imports (wired into CI/local lint).
- Naming conventions now have automated lint enforcement via `frontend/scripts/check-naming-conventions.mjs` (`npm --prefix frontend run lint` runs ESLint + naming checks).
- Architecture docs were expanded with naming/import guidance (`ARCHITECTURE_NAMING.md`, `ARCHITECTURE_IMPORTS.md`) and README cross-links.
- Fedora/GNOME Software packaging metadata was added (AppStream + desktop metadata + packaging wiring) to improve distribution readiness.
- Completed implementation TODO plans were archived under `docs/todo-archive/`, and remaining project text/comments were normalized to English.
- Frontend structure split: the former monolithic `App.svelte` explorer logic is now decomposed into feature hooks (`navigation`, `search session`, `file ops`, `context menu`, `input handlers`) with `ExplorerPage.svelte` as the composition root.
- Backend error flow migration was expanded across remaining modules, replacing string-based failures with code-based `ApiError` mapping in command and core subsystems.
- New domain-level error modules were introduced in core areas (`fs_utils`, `metadata`, `statusbar`, `undo`) to standardize classification and reduce ad-hoc text matching.
- Undo internals were fully migrated to typed errors and split into focused internal modules (`backup`, `engine`, `nofollow`, `path_checks`, `path_ops`, `security`, `types`, `error`) with updated tests.
- Frontend error handling now uses a shared Tauri invoke wrapper plus error normalization utilities, eliminating `[object Object]` toast output for structured backend errors.
- Drag/drop handling was moved out of `App.svelte` into dedicated explorer hooks, and backend policy resolution now owns copy-vs-move decisions for drop operations.
- URI/network classification rules were further centralized in backend command modules, reducing duplicated frontend scheme mapping logic.
- Extract-action availability is now sourced from backend command capabilities instead of frontend extension-only heuristics.
- Backend source layout was tightened with additional modular splits across commands/core (for example `open_with`, `clipboard`, `undo`, `fs/trash`, `context_menu`, and related command domains).
- Explorer wheel scrolling was simplified and stabilized: a single always-on wheel assist strategy now owns scrolling in list/grid, with centralized tuning and deterministic handling of non-cancelable wheel bursts.
- Explorer wheel/rendering behavior under extreme wheel input was tuned by reducing per-event max step, increasing list/grid virtualization overscan, and snapping scroll targets to integer pixels to reduce transient flicker/half-tone artifacts.
- Sorting behavior and performance were refined: `Size` sorting now keeps files before links before directories in both directions, directories sort by item count in the `Size` column, the unused `Starred` sort path was removed, backend sort keys are cached more aggressively, and frontend in-memory search sorting now uses cached/decorated keys for large result sets.
- Explorer state internals were further decomposed into dedicated `state/*` modules (`searchSort`, `entryMutations`, `createSortRefreshDispatcher`, `searchRuntimeHelpers`, `createSearchSession`) while preserving the public `createExplorerState` API, plus a new `frontend/src/features/explorer/state/README.md` documents folder boundaries.
- Modal shell behavior was hardened: immediate `Esc` close now works even before modal content receives focus, `Esc` close handlers no longer double-fire via bubbling, and focus is restored to a sensible prior element when modals close.
- Modal keyboard defaults were improved: duplicate-check search can start with `Enter` from the search-root field, confirm/conflict/delete dialogs now support immediate `Enter` actions via explicit default focus policies, and duplicate in-input `Esc` handlers were removed where `ModalShell` already handles closing.
- Conflict handling correctness was fixed for drag-move auto-rename: choosing `Auto-rename` on name conflicts no longer overwrites existing targets during move operations on Linux/Unix paths; backend now preserves no-overwrite behavior so rename-candidate retries can run.
- Properties modal UX and stability were improved: ownership moved to a dedicated tab, permission toggles are temporarily disabled during async apply (without flashing the whole permissions pane), the layout was tightened/resized/responsive-tuned, and owner/group dropdowns can overflow beyond modal bounds when needed.
- A custom shared slider UI component was added (square thumb + square track) and wired into settings/compression controls, replacing native range styling inconsistencies.
- Advanced Rename preview updates no longer visibly flicker/repaint the modal while typing; preview now updates in place without swapping the preview pane content.
- Search now supports a scoped AQS-lite query syntax in backend streaming search (`AND`/`OR`/`NOT`, grouping, wildcards `*`/`?`, quoted exact phrases, exact-value `=`, and field filters for `name`, `filename`, `folder`, `path`, `hidden`, and `readonly`), and search-mode frontend filtering no longer re-filters backend AQS results as plain text.

## v0.4.4 — 2026-02-17
- Destructive move hardening: removed Linux check-then-rename compatibility fallback when `renameat2(RENAME_NOREPLACE)` is unavailable; operations now use a controlled non-overwrite copy+delete fallback with explicit narrower (non-atomic) guarantees.
- Windows/portable destructive-op hardening: Windows rename path now uses the native move API with explicit destination-exists mapping, and non-Linux recursive delete now validates no-follow metadata recursively instead of calling raw `remove_dir_all`.
- Archive extraction hardening: Linux extraction now uses descriptor-based no-follow directory/file primitives across tar/zip/7z/rar and single-file decompress paths to reduce symlink and path-race exposure.
- Archive safety limits are now disk-aware: effective extraction byte cap is computed from available destination disk space with a 1 GiB reserve, plus periodic runtime free-space checks during writes.
- Clipboard copy hardening: fallback copy now uses no-clobber file creation (`create_new`), and rename conflict handling uses deterministic candidate retries without pre-`exists()` probing.
- Duplicate scan pressure controls: collection now enforces scanned/candidate file caps and iterates `read_dir` streams directly (no full directory-entry buffering).
- Properties modal ownership editing now uses searchable User/Group dropdowns populated from discovered system principals.
- Wastebasket list mode now resolves icon type from original item metadata so entries show file-type-specific icons instead of a generic file icon.
- Keyboard UX: `Esc` now exits both search mode and filter mode directly to breadcrumb view (address mode with unfocused path input).
- Address mode UX: pressing `Esc` while editing the path now restores the current valid location path before returning to breadcrumbs.
- Filter mode UX: pressing `Enter` is now a no-op (it no longer triggers path navigation).
- Clipboard UX/performance: large-selection `Ctrl+C`/`Ctrl+X` now use path-based flows that avoid quadratic selection scans.
- Context-menu and delete flows were optimized for large selections (Set/Map lookups and reduced repeated selection reconstruction).
- Clipboard/file-operation shutdown handling was hardened to reduce late-stage work and event emissions during app exit.
- Input/search refactor: mode transitions (`address`/`filter`/search session) are now centralized for more consistent state resets.
- Search state cleanup: `searchRunning` now represents active backend search execution, with state ownership moved to the explorer state layer.
- Wastebasket delete performance: trash entries are now resolved and purged by stable trash IDs, reducing unnecessary `.trashinfo` scans.
- Wastebasket reliability/security hardening: Unix trash/undo rename-delete paths now use descriptor-based no-follow primitives to reduce symlink and check-then-use race exposure.
- Wastebasket compatibility: no-overwrite rename now falls back on Linux when `renameat2(RENAME_NOREPLACE)` is unavailable, with documented narrower race guarantees instead of hard failure.
- Wastebasket crash recovery: staged trash renames are now journaled and recovered on startup if a previous trash operation was interrupted.
- Windows wastebasket correctness: trash moves no longer use staged renames on Windows, so restore keeps the original path and filename.
- Wastebasket internals were refactored behind a backend abstraction and covered with rollback/fallback/cleanup unit tests.
- Linux Open With hardening: selected app IDs are now resolved only from canonical in-scope `.desktop` files (symlink/out-of-scope entries are rejected).
- Properties permissions: ownership editing (`user`/`group`) now supports privilege escalation on Linux via `pkexec` helper fallback when needed.
- Permissions/ownership behavior: changes from the Properties modal are now intentionally excluded from undo/redo history.
- Permissions/ownership safety: rollback paths are now decoupled from undo action types and validated with dedicated partial-rollback failure tests.
- Properties modal polish: permissions/ownership layout now follows density-aware sizing (cozy/compact), with a smaller ownership apply button.
- Error readability: long error messages now wrap in modal error pills, properties ownership errors, and notice banners.
- Frontend ownership flow now suppresses expected warning-noise in dev logs (for example auth dismissal or unknown user/group validation errors).
- App logs now use local timestamps with timezone offset (for example `+01:00`) instead of UTC `Z` formatting.
- Network layer was split into dedicated backend/frontend modules (`src/commands/network/*`, `frontend/src/features/network/*`) and wired into the `Network` view lifecycle.
- Network discovery now aggregates GVFS (`gio mount -li`), Avahi/mDNS, and SSDP sources to surface broader SFTP/SMB/NFS/FTP/WebDAV/AFP/HTTP/HTTPS endpoints.
- Address bar + URI handling now supports broader server-address aliases (`ssh`→`sftp`, `webdav`/`webdavs`→`dav`/`davs`, `ftps` accepted as FTP-family alias for normalization/matching).
- Mount UX now reports explicit outcomes (`Connecting`, `Already connected`, `Connected`, `Failed`) from backend to frontend activity labels.
- GVFS mount visibility checks were hardened with retries and stricter mounted-URI validation.
- Linux partitions now hide the generic GVFS root mount while still surfacing concrete GVFS endpoints (for example active MTP mounts).
- Network context menu is now URI-aware: mountable URIs show `Connect`/`Copy Server Address`, HTTP(S) URIs show `Open in Browser`, mounted paths keep `Open`/`Disconnect`.
- Properties modal now supports virtual network URIs in the Extra tab by showing parsed URI fields (address/protocol/user/host/port/path/query/fragment) without failing filesystem metadata probes.
- Column-filter UX was refined: facet staleness/parity issues were fixed, active filter indicators were improved for both list and grid modes, and grid now shows an explicit active-filter notice when headers are hidden.

## v0.4.3 — 2026-02-13
- Added a topbar main action menu (hamburger) with wired actions for Settings, Keyboard Shortcuts, Search, view-mode toggle (List/Grid), hidden-files toggle, Refresh, and About.
- Added a dedicated About modal with three tabs: `Version` (embedded changelog), `Build` (runtime/build target details), and `License`.
- License tab now shows both `LICENSE` and `THIRD_PARTY_NOTICES` in one combined scrollable text field.

## v0.4.2 — 2026-02-13
- Column filters now apply real filtering on top of text filter/search, with name/type/modified/size buckets, reset via right-click, and red active indicators.
- Size/modified/type filter options are sourced from the current listing or backend column sets; hidden files are respected and size buckets skip folders.
- Settings: `Double-click speed` is now wired to actual list/grid mouse-open behavior and persisted as a validated preference.
- Settings: Added Data maintenance actions to clear thumbnail cache, stars, bookmarks, and recents with confirmation dialogs and per-action toasts.
- Thumbnail cache clear now removes cached files on disk and refreshes visible thumbnails in the UI.
- Properties modal: Extra metadata is now lazy-loaded when the **Extra** tab is opened (no eager metadata fetch on modal open).
- Extra metadata backend reorganized into type-specific providers (image, pdf, audio, video, archive) and no longer duplicates Basic-tab fields.
- Extra tab UI simplified by removing the redundant `kind` row and section headings; it now focuses on the metadata fields directly.
- Image extra-metadata routing now includes `.tif` and `.tga`.
- Image extra-metadata routing now includes `.hdr` and `.exr`.
- Bundled Linux PDFium updated to `146.0.7678.0` (including refreshed `libpdfium.so`, headers, and license set).
- Linux open-console launcher now uses a strict terminal allowlist and fixed arguments (removed env-driven terminal command overrides).
- Extraction safety guardrails expanded: total output cap (`100 GB`) and total-entry cap (`2,000,000`) are both enforced.
- `RAR` extraction now streams entry data in chunks instead of buffering whole entries in memory.
- Clipboard helper binaries (`wl-copy`, `wl-paste`, `xclip`) and `ffprobe` now resolve through canonical path checks before process spawn.

## v0.4.1 — 2026-02-08
- Linux rendering fallback simplified: when hardware acceleration is disabled, Browsey now sets only `WEBKIT_DISABLE_DMABUF_RENDERER=1` (removed legacy compositing/software flags).
- Added **Check for Duplicates** tool in the file context menu (single-file selection), with a dedicated modal built on the shared modal shell and app-wide modal styling.
- Duplicate scan backend: two-stage matching (file size pre-filter, then byte-for-byte compare with early mismatch exit), symlink-safe traversal, and deterministic sorted output.
- Duplicate scan UX: streaming progress, modal progress bar, clean cancellation when closing the modal (including Esc), and improved result preview/copy behavior.
- Docs: README updated with duplicate-check behavior and current Linux hardware-acceleration policy.

## v0.4.0 — 2026-02-06
- Hardened undo backup cleanup: validates undo dir location before deleting contents and avoids deleting the root folder outright.
- Search command now runs in a blocking task to prevent UI freezes on large trees.
- Asset protocol scope made portable via cache-dir placeholder, restoring thumbnail access across machines.
- Archive handling: added 7z and RAR extraction, batch extract with shared progress/cancel and undo, plus safer single-root handling and ZIP level/name persistence.
- Thumbnails: switched to PDFium backend with bundled binaries, faster decode pipeline (pool scaling, retries, timeouts), two-generation cache and video-thumb preference; PDF caching and resource lookup fixed.
- GVFS/MTP: better mount detection, polling/cancel/debounce to avoid UI hangs; copy/move now supports progress, cancel, and gio fallback; clearer cloud labels and icons.
- Scroll/viewport perf: list scroll and wheel events now rAF-throttled; entry-meta refresh batched to reduce jank.
- Drag/drop & clipboard: native file drop support with correct copy/move hints; system clipboard cut/copy integration and conflict modal readability improvements.
- Settings & UX: persistent defaults for view, start directory, folders-first, hidden-last, show hidden, confirm delete, density (cozy/compact); cleaned settings UI and removed unused theme/icon controls.
- Docs: README notes inspiration from GNOME Nautilus; version bumped to 0.4.0.

## v0.3.0-beta1 — 2026-01-25
- Added thumbnail pipeline with caching, format allowlist, permission checks, decode timeouts, and global concurrency limits.
- Grid view now loads thumbnails lazily via IntersectionObserver + queue; falls back to icons instantly on error.
- Asset protocol scope enabled for loading cached thumbnails; cache trimming with size/file caps.
- UI tweaks: larger grid icons, tighter card spacing, custom file-name tooltips, and refined theme toggle spacing.
- Dependency updates and safety hardening around path canonicalization, symlink/device rejection, and symlink-safe temp paths.

## v0.2.0-beta1 — 2025-01-18
- New custom icon set for folders/files/status; refreshed bookmarks, network, trash, and drive icons.
- Theme toggle redesign with clearer affordance and spacing.
- Grid/list polish: badge placement, spacing adjustments, and smoother scrolling.
- Maintenance: dependency bumps and minor fixes.

## v0.1.0-beta1 — 2025-01-11
- Initial public beta with browsing, search, bookmarks, starring, trash, compression, permissions editing, and virtualized grid/list views.
- Cross-platform support via Tauri 2 with Svelte/TypeScript frontend and Rust backend.

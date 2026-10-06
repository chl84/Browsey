# Browsey Native Test Suite TODO

Created: 2026-10-05
Status: Active suite development and scoped native verification.

Goal: Build reusable tests of basic file operations and edge cases in a real
Browsey window, using WebKitGTK, the Rust backend and independent checks of
generated files. Start with local disk, USB, network, OneDrive and MTP.
This is a test-suite execution plan, not a list of missing Browsey features.

## Existing foundation and related documents

The [native suite guide](../testing-native.md) covers the separate candidate,
private profile, approved-root configuration, WebDriver, AT-SPI and reporting.
Shared create, rename, permanent-delete, copy/move and local copy undo/redo cases
already exist in `frontend/e2e-native/cases.mjs`. Extend these rather than building
parallel suites for each provider. Policy tests and backend path-access checks
have passed; a complete five-provider native UI run has not been accepted.

Expected operation semantics remain in the
[core operations matrix](../operations/core-operations/matrix.md) and its
[release checklist](../operations/core-operations/release-checklist.md).
Accepted candidate evidence belongs in the
[daily-driver validation checklist](../operations/linux-release/daily-driver-validation-checklist.md)
and linked run records. This plan does not redefine those contracts or close
their parent rows automatically.

## Safety and completion rules

Only existing, explicitly approved roots named exactly `ai_agent_testfolder`
may contain test data. Concrete approvals stay in the ignored local config,
not this document. Generate fixtures inside an exclusive `.bnt-*` run; never
discover roots, inspect personal files or reuse existing test-folder contents.
Use private credentials and settings as described in the suite guide.

This plan does not authorize formatting, filling, ejecting or disconnecting real
devices; stopping shared GVFS/network services; controlling personal windows;
changing desktop settings/MIME defaults; or inspecting/purging personal trash.
Use bounded, fixture-contained fault injection where possible. Broader desktop
and lifecycle tests remain BLOCKED until their separate scope is approved.
Agree byte, entry-count and duration limits before large or repeated workloads;
never exhaust a real disk, account quota or phone.

Check a suite-development item when its deliverable and focused verification
are complete. Check a functional item only after reusable coverage and real
native verification for its declared provider scope are recorded. Mock passes,
policy tests and delivered clicks are not native acceptance. Do not rebuild
existing implementations merely because their acceptance remains incomplete.

Record candidate commit/dirty state, binary/harness hashes, host/tool versions,
item/case IDs, providers, results and exclusions. Each required provider/case
needs its own PASS, FAIL or BLOCKED result. DEFERRED and NOT_CONFIGURED are not
passes; N/A requires a capability-based reason and a tested explicit UI outcome.
Partial verification leaves functional items open. Update boxes and evidence
after each completed increment, not only after finishing a priority group.
Raw reports, screenshots, tokens and machine-specific paths remain private;
publish only redacted summaries in the repository.

## Priority 0 Stabilize the existing harness

Complete these prerequisites before expanding the functional matrix.

- [x] **NT0-1** Rebuild the current candidate and complete the existing local
  foundation, then all five approved providers. Verify binary/session identity
  and reject stale artifacts; never fall back to installed Browsey.
  Verified 2026-10-05 in the
  [foundation follow-up](../operations/linux-release/runs/2026-10-05-native-foundation-fixes.md):
  one five-provider run passed 43 cases, including every provider foundation and
  all eight ordered local-hub file/tree copy/move routes, then failed the history
  wait. After correcting only that wait, a fresh local run passed all eight cases,
  including undo/redo, on the same binary/build inputs. Declared case coverage is
  complete across these independent runs; the original full report remains FAIL,
  not retroactively PASS. Identity checks reject stale source/executable/commit.
- [x] **NT0-2** Extend scope regressions for traversal, prefix siblings, symlink
  replacement, ambiguous paths and rejected commands. Verify denial before
  outside I/O, using synthetic paths and owned fixtures only.
  Verified 2026-10-05: all six Rust scope regressions and all nineteen Node policy/
  orchestration regressions passed. Denied mixed requests reach zero metadata
  callbacks; actual replaced/nested/broken links are rejected in generated
  fixtures. Context-menu selections are checked and network trash is explicitly
  denied. This does not claim atomic protection against concurrent path swaps.

- [x] **NT0-3** Stabilize input with one keyboard layout (maintainer scope update
  2026-10-05; both Norwegian and US are not required): verify exact
  field values containing `/`, `_` and Unicode before submission, focus and
  modifier release. Do not change the user's global layout without authorization;
  the maintainer explicitly permits temporary US here, with restoration afterward.
  Verified 2026-10-05 in the real scoped candidate with Norwegian: `input-local`
  checks address `/`, `_`, `æøå`, modal creation/rename, exact values and focus
  before Enter, then independent file bytes and old/new paths. W3C key actions
  explicitly press/release Shift for `_` and always release modifiers. Temporary
  US did not solve driver input; original Norwegian configuration was restored.
  Non-BMP emoji input remains excluded; NT4-1 is not accepted by this check.
- [x] **NT0-4** Harden startup/teardown for occupied ports, driver exit, candidate
  crash, failed session closure and timeouts. Stop only owned processes/windows;
  uncertain teardown must not produce PASS.
  Verified 2026-10-05 in the
  [lifecycle run](../operations/linux-release/runs/2026-10-05-native-lifecycle.md):
  all 30 policy/orchestration tests passed, including occupied/foreign ports,
  spawn/exit, startup timeout, TERM resistance, changed identity and unconfirmed
  teardown. Five fresh local native fault runs caught their exact injected
  failure and confirmed captured owned process exits; those reports retain
  FAIL/BLOCKED. A normal local retest passed all eight cases and all four teardown
  steps. Ownership covers tauri-driver, its WebKit child and the scoped candidate;
  uncertain teardown blocks PASS without resending operations/session closure.
- [x] **NT0-5** Add per-case/provider capabilities and results, including setup
  failures and partial batches. Distinguish harness failures from reproduced
  app defects; never automatically retry an uncertain mutation.
  Verified 2026-10-05 in the
  [reporting run](../operations/linux-release/runs/2026-10-05-native-reporting.md):
  schema 3 declares per-case/provider requirements, setup stages, capability
  evidence and separate file/tree parts. All 40 policy tests passed, including
  partial provider setup, directional requirements on all five providers and
  distinct app/result/harness/unknown failures. Missing-driver, setup and partial
  copy faults retained BLOCKED/NOT_RUN and completed parts without retry. Normal
  local and local/USB runs passed 8 and 17 cases; mobile preflight correctly
  blocked all 17 planned cases before writes because its approved path was absent.
  This completes reporting coverage. The later
  [mobile retest](../operations/linux-release/runs/2026-10-05-native-mobile-retest.md)
  passed all 17 local/mobile cases and 16 file/tree transfer parts on clean commit
  `9868e129`, including copy/move in both directions. The earlier preflight remains
  BLOCKED; device lifecycle acceptance remains outside this foundation scope.
- [x] **NT0-6** Verify credential/artifact permissions and Git exclusions. Define
  bounded retention and explicit owned-run cleanup, preserving recovery data
  and never recursively cleaning an approved root.
  Verified 2026-10-05 in the
  [privacy/retention run](../operations/linux-release/runs/2026-10-05-native-retention.md):
  all 58 policy tests passed; fresh local acceptance passed eight cases and the
  reporting/lifecycle fault assertions retained private recovery evidence.
  Private metadata, bounded local accounting and explicit UUID/plan-hash cleanup
  are enforced. Cleanup deletion passed only on synthetic fixtures; real old,
  failed/blocked, multi-provider and runtime-socket runs remain retained.

## Priority 1 Basic operations and navigation

Use shared cases for applicable providers and independently verify actual files.

- [x] **NT1-1** Navigate owned folders in list/grid, including empty folders,
  in-scope breadcrumbs, back/forward, menu Refresh, F5 and repeated visits.
  History/bookmarks must not escape the owned session.
  Verified 2026-10-05 in the
  [navigation run](../operations/linux-release/runs/2026-10-05-native-navigation.md):
  all five providers passed both views, 80 recorded parts and independent
  generated-file readback. Owned accessibility, fullscreen, process teardown
  and private retention passed. Menu handling and stale OneDrive refresh were
  corrected and retested; F5 remains enabled and prevents the WebView default
  before awaiting refresh. All 65 harness tests, 650 frontend tests and 761 Rust
  tests passed (19 Rust tests ignored). Watchers, ancestor navigation, narrow
  layout and device lifecycle remain outside this scope.
- [x] **NT1-2** Verify sorting, column filters/reset, hidden generated files and
  in-scope search. Cover empty results, case/extension differences and mode
  changes without stale filters or lost folder identity.
  Verified 2026-10-05 in the
  [listing/search run](../operations/linux-release/runs/2026-10-05-native-listing.md):
  all five providers passed 75 parts, owned accessibility, independent byte
  readback and teardown/retention. Recursive OneDrive search is implemented and
  verified alongside local/GVFS search. Missing start directories now fail
  instead of searching home; unsubmitted drafts clear old results. All 71
  harness tests, 654 frontend tests and 768 Rust tests passed (19 Rust tests
  ignored). Watchers, large-search performance, timezone/date-range boundaries
  and other cloud providers remain outside this scope.
- [x] **NT1-3** Verify single/multiple selection, Ctrl/Shift, arrows and select-all,
  virtualized rows, empty-space clicks and navigation. Operations must receive
  exactly the intended entries once.
  Completed by the [native selection verification](../operations/linux-release/runs/2026-10-05-native-selection.md):
  all 60 list/grid parts passed on local disk, USB, network, OneDrive and mobile,
  plus five parts on a representative 200-file local virtualized list. Exact
  selected-copy membership/bytes, source preservation, owned accessibility,
  teardown and private audits passed. Final reporting changes passed a fresh
  local retest. All 78 harness tests and 36 relevant frontend tests passed.
  Remote large-list/grid virtualization and repeated operation requests remain
  outside this scope; NT1-6 covers repeated dispatch. All real runs are retained.
- [x] **NT1-4** Create files/folders with empty contents, duplicate names and
  invalid input. Test Enter, Cancel/Escape and focus restoration; rejection
  must leave existing fixtures unchanged.
  Completed by the [native creation verification](../operations/linux-release/runs/2026-10-05-native-creation.md):
  all 120 parts passed on local disk, USB, network, OneDrive and mobile, including
  independent contents/membership preservation, corrected drafts, modal focus,
  owned accessibility, teardown and private audits. Invalid leaf names, cloud
  folder collisions and stale retry errors are fixed. All 83 harness tests,
  657 frontend tests and 770 Rust tests passed (19 Rust tests ignored). Concurrent
  writers, platform-specific reserved names and other keyboard layouts remain
  outside this scope; all real runs are retained.
- [x] **NT1-5** Rename files and non-empty folders, checking nested contents,
  collisions, extensions, case-only changes, cancellation and repeated submission.
  Verify old/new paths and preservation of unrelated fixture files.
  Verified by the [editing/rename runs](../operations/linux-release/runs/2026-10-05-native-editing-history-properties.md):
  all 60 parts passed across local, USB, network, OneDrive and mobile, preserving
  exact nested/unrelated bytes after every attempt. OneDrive case-only file/folder
  rename and MTP duplicate-entry handling are corrected and retested; collisions,
  Cancel/Escape, repeated Enter and corrected rejection pass. All owned teardown
  stages and fresh private audits passed; original failures remain retained.
- [x] **NT1-6** Revalidate copy/cut/paste and deletion through keyboard/context
  menus: mixed/no selection, rapid repeated requests, permanent-delete warnings
  and cancelled confirmation without duplicated operations.
  Verified by the [editing/file-operation runs](../operations/linux-release/runs/2026-10-05-native-editing-history-properties.md):
  all 40 parts passed across local, USB, network, OneDrive and mobile with exact
  generated-tree/byte readback. Mixed context selection and permanent-delete
  warnings are fixed; repeated requests and Cancel/Escape preserve the expected
  state. Owned accessibility, teardown and fresh private audits passed. Original
  stopped reports remain retained with explicit scoped acceptance.
- [x] **NT1-7** Expand local undo/redo to move, rename, supported deletion and
  overwrite. Verify redo invalidation, the documented 50-action limit and restart
  semantics; do not imply cloud undo or persistent history.
  Verified by the [editing/history run](../operations/linux-release/runs/2026-10-05-native-editing-history-properties.md):
  all 108 history parts passed on the final candidate, including move, rename,
  nested permanent deletion, overwrite, redo invalidation and the exact 50-action
  boundary. Both Undo and Redo were populated before one owned restart and
  unavailable afterward; retained fixture bytes/paths remained correct. All six
  old/new captured processes exited and fresh private retention audit passed.
  History remains local and limited to the active app session.
- [x] **NT1-8** Verify Properties for generated files/folders: correct selection,
  size/type, ownership/permission capabilities, tabs and focus. Do not inspect
  drive roots or personal metadata.
  Verified by the [editing/Properties runs](../operations/linux-release/runs/2026-10-05-native-editing-history-properties.md):
  all 15 file/folder/mixed parts passed across local, USB, network, OneDrive and
  mobile. Known totals/counts, unknown cloud directory totals, all four tabs,
  ownership/permission capabilities, focus and independent preservation passed.
  No permissions were changed; owned teardown and fresh private audits passed.

## Priority 2 Transfer matrix and conflicts

For five providers, the local hub has eight ordered cross-provider routes;
all-pairs has twenty. Test copy and move separately for files and directory trees.

- [x] **NT2-1** Verify within-provider copy/move on every approved target with
  empty folders, nested trees and mixed batches. Compare destination bytes/tree
  and source preservation/removal independently of UI feedback.
  Completed by the [native transfer verification](../operations/linux-release/runs/2026-10-06-native-transfers.md):
  all 40 file/empty-folder/nested-tree/mixed-batch parts passed on local, USB,
  network, OneDrive and mobile for both copy and move. Exact source/destination
  trees, nested/unrelated bytes, accessibility, owned teardown and fresh privacy
  audit passed. Empty cloud directory copies are fixed and retested; the original
  blocked report remains retained. Rust passed 779 tests (19 ignored), native
  policy passed 99, and Clippy/native lint/docs checks passed.
- [x] **NT2-2** Run both directions between local disk and USB, network, cloud
  and mobile. Successful reading does not establish writing or moving back.
  The [accepted hub run](../operations/linux-release/runs/2026-10-06-native-transfers.md)
  passed all 16 mixed file/empty-folder/nested-tree operations across all eight
  ordered routes. Both exact trees, source preservation/removal, accessibility,
  owned teardown and a fresh audit passed. The generated-file permission audit
  preserves inherited copy modes under private parents; all 103 policy tests
  and native lint pass. The original blocked audit report remains retained.
- [x] **NT2-3** Add bounded all-pairs runs, including network/mobile and USB/cloud.
  Record routing, staging and explicit refusals; an unrun route is not covered
  by the local hub.
  The [all-pairs native run](../operations/linux-release/runs/2026-10-06-native-transfers.md)
  passed all 40 copy/move mixed-tree parts across all 20 ordered routes on one
  candidate. Every part retains selected-backend/direct-destination receipts
  plus exact independent source/destination trees. No route is refused/unrun;
  accessibility, owned teardown and fresh privacy audit passed. Provider-internal
  temporary files are outside the routing claim. Policy 105/Rust 782 tests and
  Clippy/native lint passed.
- [x] **NT2-4** Cover skip, overwrite, unique-name and cancel conflict choices:
  files, directories, nested collisions and file-versus-directory conflicts.
  Assert the documented policy and both sides' bytes after each result.
  The [native conflict verification](../operations/linux-release/runs/2026-10-06-native-transfers.md)
  completed all 56 cases/112 parts on the five providers and both local/cloud
  directions. All four choices preserve the documented source/destination trees;
  cloud/mixed cross-kind overwrite explicitly refuses, and cloud folder moves
  retain empty destinations and remove their empty source roots. A corrected
  independent audit and fresh 32-part local/network integration run pass on the
  same app/binary; the original audit-blocked report retains its status. Owned
  teardown/accessibility and frontend 678/Rust 786/policy 109 checks pass.
- [x] **NT2-5** Reject same-target operations, transfers into descendants and
  aliases that would recurse or destroy sources. Verify deterministic errors
  and bounded work, not only disabled controls.
  The [native guard verification](../operations/linux-release/runs/2026-10-06-native-transfers.md)
  passed all 47 declared parts across local, USB, network, cloud and mobile, combining
  unchanged USB/network results with the nine ordinary cloud parts plus a fresh 11-part cloud-alias/mobile run on
  the same app/binary. Unsafe operations reject visibly and preserve whole trees;
  valid same-parent copies remain unique. Source-alias and bounded-work regressions,
  private audits, accessibility and owned teardown pass. The initial cloud-fixture
  setup failure remains BLOCKED and retained separately.
- [x] **NT2-6** Exercise a failing entry in a batch: accurate completed/skipped/
  failed counts, remaining sources, refresh and error messages. Success must
  not hide a transfer or subsequent refresh failure.

  The [native batch verification](../operations/linux-release/runs/2026-10-06-native-transfers.md)
  passed eight representative cases on local, USB and cloud: actual generated
  local read denial, partial upload/cut, bounded cloud-source failure and exact
  one-use refresh failures after success and failure. Counts, skipped/unattempted
  roots, whole trees, remaining cut paths and combined error feedback pass.
  Cloud injection verifies reconciliation rather than provider I/O outages;
  network/mobile failure modes remain outside this scope. Private audits,
  accessibility, owned teardown and production-core checks pass.

## Priority 3 Progress cancellation and failure safety

Prioritize these data-safety cases before cosmetic UI coverage.

- [x] **NT3-1** Verify visible byte progress in both directions across boundaries:
  files/folders, unknown totals, slow callbacks, zero-byte files and finalization.
  Prevent misleading `1 B`, early completion and stale activity.
  Scoped native PASS: nine local-hub routes, 27 file/folder/zero-byte parts,
  including both cloud and mobile directions. Real local stream increments
  and bounded finalization pauses expose displayed state; unknown cloud totals
  remain indeterminate. Large files and universal mid-file callback cadence
  are excluded. See the [NT3 verification record](../operations/linux-release/runs/2026-10-06-native-failure-safety.md).
- [x] **NT3-2** Cancel copies before I/O, mid-file and between files. Independently
  verify writes stop, sources survive, destination state and partial-result
  messages are accurate, and listeners are released. Cancel is not rollback.
  Scoped native PASS: 13 cases, all nine hub routes before writing, a real
  local 16 KiB mid-file stop, and local/bidirectional cloud between-file stops.
  Two independent tree comparisons verify quiescence and source preservation;
  actual task tokens and Tauri callbacks are released. Large-file and mid-file
  GIO/cloud timing remain outside this representative scope. See the NT3 record.
- [x] **NT3-3** Cancel/fail overwrite of a file with different existing bytes.
  Verify the documented overwrite/recovery boundary: no silent loss or incomplete
  output reported as complete. Inspect retained backups/diagnostics without
  assuming transactional directory operations.
  Scoped native PASS: nine local/USB/cloud cases; pre-write cancellation and
  unreadable-source failures preserve original targets, while local/USB mid-file
  stops retain incomplete outputs and exact protected backups with diagnostics.
  No cloud/GIO mid-file rollback or directory transaction is claimed. See the NT3 record.
- [x] **NT3-4** Cancel/fail moves within/across providers. Remove sources only
  for successfully completed entries and report partial batches truthfully,
  without automatic destructive retries.
  Scoped native PASS: 22 cases, all five providers within and all hub directions
  before writes, USB mid-file and USB/cloud pre-delete stops, local/cloud batch
  cancellation and representative USB/cloud failures. Exact trees, remaining
  cut selections, counts and released callbacks match. Local upload moves now
  recheck cancellation and original source version before unlinking. See the NT3 record.
- [x] **NT3-5** Exercise read/write denial and read-only behavior on owned fixtures.
  Require actionable errors and disabled unsupported chmod/chown controls,
  rather than assuming every filesystem supports Unix permissions.
  Scoped native PASS: 15 file/folder/mixed Properties parts across all five
  providers, plus three actual local Unix read/write-denial/read-only-copy parts.
  Exact bytes and unrelated sentinels match; unsupported ownership/access
  controls and explanations are checked without changing ownership or mounts.
  Unix denial is representative local coverage, not a claim for every provider.
- [x] **NT3-6** Simulate full destination, unavailable provider and transient I/O
  faults with bounded fixture-scoped injection or an approved sandbox. Never
  fill physical media or stop shared services to reproduce failure.
  Scoped native PASS: five exact-path, one-use local/USB/cloud I/O faults and
  four real copy/cut cancellations during cloud destination preparation.
  Sources and unrelated bytes match; exact retained 16 KiB prefixes, counts,
  stopped writes and released task/callback registries are independently checked.
  Cloud/mixed preparation now retains cancellation intent and uses a cancellable
  metadata read. Actual outages/quota exhaustion and other provider fault phases
  are outside this representative scope. See the NT3 record.
- [x] **NT3-7** Change/remove sources or destinations during work using owned
  fixture writers. Cover races, symlink swaps and late collisions; verify source
  retention and truthful uncertainty without claiming complete atomicity.
  Scoped native PASS: six local/USB/cloud writer races, including same-size
  mid-copy/move edits, deliberate source removal, an owned source symlink swap,
  a late overwrite collision and a source edit after completed cloud upload.
  Exact changed/mixed/full bytes, unrelated sentinels, retained cut selections,
  foreign destination bytes and marked original backups match independently.
  The writer restores only its own temporary link after the task has stopped;
  externally removed sources are expected absent. See the NT3 record.
- [x] **NT3-8** Interrupt/restart only the owned candidate during operations.
  Inspect partial outputs and private recovery data; verify safe startup and
  diagnostics, not power-loss protection or automatic resume.
  Scoped native PASS: one local overwrite interrupted by SIGKILL after 16 KiB,
  followed by confirmed owned teardown and one restart of the same private profile.
  Three parts preserve exact source/partial target/original backup/marker bytes,
  check idle startup with empty history and no replay, and exercise the actual
  read-only Settings recovery panel. Old/new captured process identities are gone.
  No provider/service/installed-app process is interrupted. See the NT3 record.

## Priority 4 Names files and directory edge cases

- [x] **NT4-1** Exercise spaces, Norwegian characters, emoji, combining Unicode,
  quotes, `#`, `%`, `&`, `_`, leading dots/hyphens and valid trailing whitespace.
  Verify URI encoding and byte/name preservation across operations.
  Scoped native PASS: 29 unique parts across all five providers, using the same
  candidate in a full run and a fresh local/mobile follow-up. Exact name/byte
  readback covers copy/move and existing emoji rename; local creation/rename
  preserves significant whitespace. This MTP device explicitly rejects quoted
  names without side effects. Emoji typing remains excluded. See the
  [NT4 record](../operations/linux-release/runs/2026-10-06-native-name-data-edges.md).
- [x] **NT4-2** Cover reserved names, name/path-length limits and unsupported Unix
  filename encodings where applicable. Require explicit rejection or documented
  representation, never silent corruption.
  Scoped native PASS: 20 parts on all five providers, with actual reserved-name
  outcomes and explicit overlong-name/rename-component rejection. Local cases
  verify 255/256 UTF-8 bytes, >4096-byte paths and readable listing/search errors
  for unsupported name bytes. Invalid names never become actionable lossy aliases;
  unchanged recovery bytes are retained under recorded UTF-8 names. See the NT4 record.
- [x] **NT4-3** Use zero-byte, one-byte, generated binary and differently sized
  files; verify digests/readback, not just size. Agree limits before large runs;
  sparse local files do not establish remote transfer performance.
  Scoped native PASS: 18 copy/move parts on all nine local-hub routes, with zero,
  one, 4097 and 65536 generated bytes. Independent raw bytes/SHA-256, whole
  membership, unrelated sentinels and previous copies pass. Successful cloud/mixed
  transfers now release their progress listeners. No large-file/performance claim.
  See the NT4 record.
- [x] **NT4-4** Exercise empty/deep/wide trees and large owned listings within
  entry/depth budgets. Verify bounded traversal, resource cleanup and UI response;
  begin device stress with reduced fixtures.
  Scoped native PASS: 18 unique parts across all five providers, with a reduced
  six-entry tree first, combined 25-entry/depth-eight trees, exact binary digests,
  recursive search and task/callback release. Local virtual listing selects
  first/last and copies all 200 one-byte files. Completed USB/network parts and
  fresh local/cloud/mobile/local follow-ups use the same app source/binary.
  Dispatch traversal stays bounded to 4096 entries/depth 32. See the NT4 record.
- [x] **NT4-5** Add broken/relative symlink and hard-link cases where supported.
  Referents must stay inside the owned run. Narrowly extend the currently
  rejecting guard before functional link tests; never allow outside referents.
  Scoped native PASS: five local Unix parts verify list/grid Link representation,
  explicit relative/broken-symlink clipboard rejection, independent hard-link
  copy and same-inode alias move. Exact owned paths/targets and two known aliases
  remain guarded; private captured identities support fresh no-follow audit.
  Unknown/outside aliases and parent links are refused. Link-policy runs retain
  every fixture and cannot be automatically cleaned up. Other providers' link
  behavior remains deferred. See the NT4 record.

## Priority 5 Provider specific behavior

Shared cases remain authoritative. Path-access checks are prerequisites, not
completed file-operation acceptance.

- [x] **NT5-1** USB: verify actual filesystem create/rename/transfer behavior,
  errors and supported permissions. Record its type; one Btrfs result does not
  certify exFAT/NTFS. Formatting is not authorized by this plan.
  Scoped native PASS on the mounted ext4 USB: file/folder operations, both
  local transfer directions, all Properties tabs, real read/write denials and
  mode-preserving read-only copy. A harness-only setup failure is retained;
  the access cases pass in a fresh run. See the NT5 provider record.
- [x] **NT5-2** Network: verify SFTP/GIO/FUSE consistency, large-file deletion
  without content download, progress/cancellation and stale-path errors in the
  approved folder. Service-loss tests need a separate isolated fault scope.
  Native PASS on configured SFTP/GVFS/FUSE: foundation operations, both transfer
  directions, byte progress and real Cancel. A generated 32 MiB file survives
  cancelled delete, then disappears without a local undo copy. Stale rename
  fails explicitly with preserved moved-source bytes and correct refreshed
  listing. Service loss remains outside this scope. See the NT5 record.
- [x] **NT5-3** Mobile: verify MTP operations, provider latency and late metadata/
  thumbnails, stable ordering and source preservation on failure. Do not enumerate
  personal camera folders.
  Native PASS on the mounted MTP test folder: file/folder operations and both
  local transfer directions, explicit quoted-name rejection with source-byte
  preservation, cold/late generated image thumbnails, stable grid/list order
  and refreshed file Properties. No personal camera enumeration or locked-phone
  lifecycle claim. See the NT5 record.
- [x] **NT5-4** Cloud: verify OneDrive refresh, transfers, conflicts, working copies
  and quota/rate/authentication errors using generated fixtures/private config.
  Google Drive/Nextcloud need separately approved exact roots and credentials.
  Fresh OneDrive native PASS covers all fourteen declared parts. Fix explicit
  overwrite skipping different equal-size/time content, verified by byte
  readback and red/green backend regressions. Private working-copy preparation,
  new-name upload and changed-original handling pass. Quota/rate/auth are exact
  prewrite candidate faults; live outages and external editors remain excluded.
  Failed attempts and their actual outcomes remain retained. See the NT5 record.
- [x] **NT5-5** Define a separately approved lifecycle mode for first connection,
  locked phone, reconnect, disappearing mounts and busy/ejected media. Foundation
  discovery/mount restrictions must not be silently removed; mounted-folder
  success does not prove lifecycle behavior.
  The [lifecycle contract](../testing-native-provider-lifecycle.md) defines exact
  one-scenario approval, device/service identity, action plan, finite budgets,
  one-use guard exceptions, state/preservation evidence and owned recovery.
  This definition deliverable is complete; no lifecycle runner is enabled and
  all real device/service transitions remain NOT_RUN pending separate scope.

## Priority 6 Native UI and desktop interaction

Extend guards only for precise owned scope. Normal clipboard, global trash,
external launches and discovery remain disabled by default.

- [x] **NT6-1** Test drag/drop within/between owned Browsey candidates, then a
  separately scoped Nautilus fixture window. Cover multiple files, folders,
  special names, modifiers, self-drops, cancel and teardown. Verify file effects;
  reuse the [native drag guide](../testing-native-drag.md).
  The [desktop interaction record](../operations/linux-release/runs/2026-10-07-native-desktop-interaction.md)
  records real X11 native acceptance, receiver copy policies and the independently
  verified file effects. Wayland/portal move and modifiers held before pointer
  press remain outside that accepted input scope.
- [x] **NT6-2** Reproduce drag-label regressions over many rows/cards versus empty
  space: responsive following, no stuck `Cannot drop here`, no duplicated transfer.
  Measure native event/frame behavior, not mock render counts.
- [x] **NT6-3** Add clipboard and trash/restore/purge tests only after separately
  approving and demonstrating desktop isolation. Private XDG directories alone
  do not prove a shared trash/clipboard service is isolated.
- [x] **NT6-4** Verify keyboard navigation, Tab order, focused Escape handling,
  modal focus restoration and accessible names through WebDriver/AT-SPI.
  Cover interacting menus/dialogs/tooltips and slow operations.
- [x] **NT6-5** Validate private-profile themes/densities, grid zoom and generated
  thumbnails under load: corrupt/unsupported images, late metadata, stable grid
  order, filter Reset and progress/modal layout. No global theme changes.
- [x] **NT6-6** Add fixture-only watcher tests after eliminating home fallback and
  broad discovery from that mode. Cover external fixture changes, refresh during
  work and shutdown; disabled-watcher runs cannot certify watcher behavior.
- [x] **NT6-7** Add generated archive round trips: supported formats, passwords,
  wrong password, Cancel, corruption, conflicts and partial batches. Cover local/
  cloud staging and malicious entry paths without outside writes; reuse existing
  archive/security tests rather than duplicating their bodies.
- [ ] **NT6-8** Test Open With/default-program checkbox using a dummy handler and
  isolated MIME state: spaced filenames, failed launches and cancellation.
  Prove no personal association changes before enabling external launches.

## Priority 7 Repeatability automation and acceptance

- [ ] **NT7-1** Separate a short local smoke tier, provider foundation, edge cases
  and opt-in stress/lifecycle tiers. Share helpers and distinguish policy, mock
  and real native evidence without duplicate test bodies.
- [ ] **NT7-2** Repeat accepted cases with fresh/reused private profiles. Investigate
  timing/focus flakes, never hiding them with mutation retries, silent skips or
  arbitrary timeout increases. Verify teardown and retained-run limits.
- [ ] **NT7-3** Measure listing, first thumbnails, zoom, drag feedback and cancel
  latency with bounded workloads, cache state and sample counts. Agree measured
  host-specific budgets before introducing regression thresholds.
- [ ] **NT7-4** Define isolated Linux CI smoke with matching GTK/WebKit/driver
  versions and no personal accounts. Real device/provider checks remain opt-in
  on approved hosts; compilation is not device coverage.
- [ ] **NT7-5** Map case IDs to acceptance rows and maintain redacted run records.
  Add only reproduced defects to the engineering backlog; archive this track
  when its declared deliverables and verification scope are complete.

## Recommended first increment

Start with NT0-1 through NT0-5 and the existing foundation, fixing harness
failures before adding cases. Next tackle NT1/NT2 and NT3, especially cross-boundary
progress and cancelled overwrite. Continue with names/provider quirks and native
interaction regressions. Broader desktop/lifecycle/CI work needs explicit scope
decisions, not unattended expansion of authority.

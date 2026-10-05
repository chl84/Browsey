# Scoped native acceptance suite (Linux)

[Development](development.md) · [Daily-driver validation](operations/linux-release/daily-driver-validation-checklist.md)

This suite drives a separately built Browsey window in real WebKitGTK, with the
real Rust backend. Existing Playwright tests remain fast, mock-IPC frontend tests;
they do not replace native acceptance. AT-SPI supplements WebDriver for the owned
window's accessibility tree, reusing `tests/support/native_fixture_a11y.py`.
Pointer/keyboard/accessibility interaction and independent file-content checks
are complementary; neither alone proves all file-operation behavior.

## Safety and ownership

Only explicitly approved, **already existing** directories named exactly
`ai_agent_testfolder` are eligible. There is no home/mount/account discovery and
no creation of missing approved roots. Earlier approvals with other spellings
do not apply. Network/mobile targets need a working absolute GVFS/FUSE path; a
GIO URI that Nautilus can open is not proof that this local path exists. Resolve
only the exact approved folder, e.g. `gio info --attributes=standard::type URI`;
do not list device roots to search for fixtures.

Every run creates an exclusive `.bnt-<UUID-without-hyphens>/files` subtree inside each
approved root. All app mutations and verification reads are limited to these
owned subtrees, not existing test-folder contents. Private HOME/XDG directories,
undo storage, credentials, logs and screenshots live in the local run. Short
`r/` and `t/` runtime/temp directories avoid native Unix socket path-length limits.
Fixtures
are small generated text files, not personal data. No installed app is launched,
replaced, restarted or controlled.

The opt-in Cargo feature `native-test` adds a fail-closed IPC command/path guard.
Unknown commands, implicit paths, traversal and local symlinks are rejected.
Automatic mount/mobile discovery, system clipboard import/export, global trash,
formatting, connection/mount actions and external application launches are not
allowed. The directory watcher is disabled because its normal discovery/fallback
can leave the approved scope; tests explicitly refresh. Undo/redo recheck owned
local trees. These restrictions are absent from normal builds.

This is a guard for a trusted test session, **not an OS sandbox** against a
malicious same-user process replacing paths during I/O. Do not concurrently edit
the owned run. Desktop libraries, system packages and the existing D-Bus services
are still used. Do not enable this feature in installation/release workflows.

## Setup

Requires Linux GTK3/WebKitGTK 4.1, Node >=22.19, locked frontend dependencies,
`tauri-driver`, and **a WebKitWebDriver compatible with the app's WebKitGTK**.
The runner manages an external driver directly to keep session/profile ownership
explicit; see [Tauri's manual setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/).
It does not install system packages or download browsers automatically.

Copy `frontend/e2e-native/config.example.json` to the ignored
`frontend/e2e-native/config.local.json`. Enter only maintainer-approved absolute
test-folder paths, or `rclone://Remote/ai_agent_testfolder` for cloud. Leave missing
providers `null`; they remain NOT_CONFIGURED, not PASS. The local root is mandatory.

For cloud, the maintainer must supply a **test-only** `rclone.conf` containing only
the approved remote, inside the approved local folder, mode `600`. Set its path
as `rcloneConfig`. The suite copies it into its private profile; it never opens
the user's normal rclone config, imports other accounts or inherits credential
environment variables/SSH agents. Token refresh can update the private copy.
Treat both config files as credentials; never commit them or raw reports.

If the maintainer explicitly authorizes reading an exact existing personal rclone
config, use `provision-rclone.mjs --approved-existing-config SOURCE LOCAL_ROOT REMOTE`
once to extract only the approved provider into `LOCAL_ROOT/rclone.conf` (mode 600).
It never overwrites the test config, modifies the source or prints credentials.
This narrow exception does not authorize reading any other personal file. The
normal runner still accepts credentials only from the approved local test folder.

```bash
npm --prefix frontend ci
npm --prefix frontend run test:native:policy
bash scripts/dev/test-native-linux.sh --plan
bash scripts/dev/test-native-linux.sh --build
bash scripts/dev/test-native-linux.sh --check
bash scripts/dev/test-native-linux.sh --run --a11y
```

`--plan` performs no target probes/writes. `--check` probes exact local paths and
tool availability without creating fixtures; it does not test cloud connectivity.
`--run` is the explicit permission to create/mutate generated fixtures inside the
approved roots. Optional `--targets local,usb` selects a bounded subset; excluded
configured targets are DEFERRED. Configured but unavailable targets stop a run;
there is no silent skip or fallback to the installed app.

Driver paths can be explicitly set with `BROWSEY_TAURI_DRIVER` and
`BROWSEY_WEBKIT_DRIVER`; otherwise only system tool directories and
`target/native-tools/bin` are checked. A local driver can be installed with
`cargo install tauri-driver --locked --root target/native-tools`. The native
WebKit driver must be supplied separately. Ports 4444/4445 must be free.
If a Cargo shim is unconfigured, `BROWSEY_CARGO=/absolute/path/to/cargo` selects a
working toolchain for `--build` without modifying global tool settings.

## First-phase cases

The same cases run against local disk, USB, network, cloud and mobile:

- Create a file/folder through the context menu and check the actual result.
- Rename a file/folder, preserving contents and removing the old path.
- Cancel permanent deletion, verify preservation, then confirm file/folder deletion.
- Copy and move files and a nested folder within each provider; verify destination
  bytes independently, with source preservation/removal as appropriate.
- Copy and move the same fixtures in both directions between local disk and each
  other configured provider. `matrix: "all-pairs"` extends this to all ordered pairs.
- Local copy undo/redo, checking both source and destination.
- With `--a11y`, check that AT-SPI exposes the exact candidate's file list; missing
  accessibility is a failure, not a skipped successful case.

UI mutations use real controls/shortcuts, not direct file-operation IPC or
synthetic DOM events. One read-only test-only IPC handshake verifies the correct
candidate/session before any UI mutation. File setup/verification uses guarded
local filesystem operations or an explicit private-config rclone command.

Not yet covered: native drag/drop and cross-instance behavior, large-file progress
and cancellation, overwrite/recovery, archives/passwords, trash, formatting,
disconnect/reconnect, normal watcher behavior and other distributions. Small-file
success does **not** prove visible progress or cancellation. Expand these in shared
cases only after the foundation runs reliably.

## Evidence, Git and retention

The shared build is staged under ignored `target/native-test`, not a worktree or
build cache per run. Its manifest records commit, dirty state, build time, profile
and executable SHA-256. Rebuild after source edits; a dirty build is not an exact
committed-baseline acceptance run. No release/install binaries are staged here.

Each local owned run retains `report.json`, private profile, generated fixtures
and window-only failure artifacts. Reports distinguish PASS/FAIL/BLOCKED,
NOT_RUN/DEFERRED/NOT_CONFIGURED and explicitly list excluded acceptance. A PASS
means only the selected small-file foundation cases, not release signoff.
Preflight failures before run creation are printed, not presented as passing tests.
There is no automatic recursive cleanup: inspect and remove only a verified owned
UUID run, never the approved test root or unrelated files. Private profiles can
contain OAuth tokens; securely handle retained runs.

Commit source, shared tests, example config, docs and dependency lockfiles. Do not
commit machine approvals, credentials, binaries, raw reports, screenshots/logs or
fixtures. `.gitignore` covers local configs, owned run names, `ai_agent_testfolder`
contents and build artifacts. Review `git diff --cached` before committing;
ignore rules do not remove already tracked secrets. Publish only a redacted,
scope-specific acceptance summary in the existing daily-driver run records.

## Initial implementation status (2026-10-05)

Policy/orchestration tests are separate from native acceptance. On this machine,
preflight initially found the local root but no USB root at the approved path,
no WebDriver binaries and no cloud test config. Exact network/mobile GIO metadata
queries succeeded while their reported local GVFS paths did not exist. No personal
folders were searched. These are blocked acceptance scopes, not checked daily-driver
rows. Resolve environment/access prerequisites before running those providers.

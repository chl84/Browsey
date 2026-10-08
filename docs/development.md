# Development and releases

[Back to README](../README.md) · [Installation](installation.md) ·
[Project docs](README.md)

The commands below target the current Linux x86_64 checkout. They use the
repository-pinned npm Tauri CLI instead of assuming a separately installed
`cargo tauri` command. Run them from the repository root.

## Prerequisites

- Rust stable through rustup, at least 1.95 for the current dependency stack.
- Node.js LTS and npm (CI uses Node 22).
- A C/C++ toolchain, pkg-config, GTK 3, WebKitGTK 4.1, D-Bus and OpenSSL 3
  development libraries (`libssl-dev` on Ubuntu/Debian or `openssl-devel` on
  Fedora). RPM/DEB bundling also needs the platform's packaging tools.

Use the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
for your distribution. Browsey's CI additionally installs D-Bus headers and
uses patchelf/rpm for Linux bundles. The exact reproducible package list is in
[Linux Release Bundles](../.github/workflows/release-linux.yml).

PDFium binaries, headers and licenses are bundled. Versions, binding profile
and checksums are recorded in [PDFium provenance](../resources/pdfium.json),
not duplicated here. Verify resources with:

```bash
node scripts/maintenance/check-pdfium.mjs
```

For pinned native dependencies, audits and upgrades, see
[dependency maintenance](maintenance/dependencies.md).

For real WebKitGTK/backend validation with explicitly approved disposable folders,
see the [scoped native acceptance suite](testing-native.md). It complements mock
frontend E2E tests and does not operate on the installed app or personal files.

## Run and build

Install the locked frontend dependencies and start the desktop development app:

```bash
npm --prefix frontend ci
frontend/node_modules/.bin/tauri dev --no-dev-server
```

The configured `beforeDevCommand` starts Vite on port 5173. `--no-dev-server`
disables Tauri's built-in static server, not this Vite hook. Do not start a
second Vite server on the same port.

Build a production executable or Linux bundles:

```bash
frontend/node_modules/.bin/tauri build --no-bundle -- --locked
# Or, for RPM and DEB:
frontend/node_modules/.bin/tauri build --bundles rpm,deb -- --locked
```

The executable is `target/release/browsey`; packages are in
`target/release/bundle/rpm/` and `target/release/bundle/deb/`.
Tauri's build hook builds and embeds the production frontend. Do not use
`cargo build --release` alone for a distributable desktop app: it may retain
the development-server configuration. Use the
[local installer](installation.md#user-local-installation-from-source-linux-x86_64)
to stage the executable together with required runtime resources.

The older dev/build wrappers use `cargo tauri`, so they require a compatible
Cargo-installed Tauri CLI separately. The npm CLI above is already installed
by `npm ci` and stays coordinated with the repository pins.

### Windows maintenance scope

Windows code remains maintenance-only; there is no current release installer
or fresh Windows runtime signoff. A source build needs Visual Studio C++ build
tools, WebView2, Rust and Node, and platform-specific Tauri configuration.
The checked-in shared configuration currently has Bash hooks and a Linux
PDFium sidecar. Adapt those hooks/resources for Windows before attempting an
NSIS build; the old `.bat` wrappers alone are not a verified Windows recipe.
macOS is not currently supported.

## Checks

```bash
cargo check --locked
npm --prefix frontend run check
bash scripts/maintenance/test-both.sh --strict-docs
```

The full maintenance suite requires the usual build dependencies, Bash,
frontend npm dependencies, and Playwright Chromium. Install the browser for
local runs with `npm --prefix frontend run test:e2e:install`. Native/session,
real-provider, device and performance tests are opt-in and are not certified
by ordinary mocked browser or Rust unit tests.

Check the separate documentation app with:

```bash
npm --prefix docs-site ci
npm --prefix docs-site run lint
npm --prefix docs-site run check
npm --prefix docs-site test
npm --prefix docs-site run build
```

Disposable performance workloads are opt-in:
`bash scripts/dev/performance-workloads.sh --dry-run`, then run without that
option on the selected filesystem. See the
[workload guide](audits/daily-driver/performance-workloads.md) for isolation,
measurements and native-candidate checks.

## Version bumps and publication

A completed Linux release includes a tested tag, production RPM and DEB
packages, verified `SHA256SUMS` and a published GitHub release containing all
three assets. Updating version metadata alone is release preparation. RPM/DEB
building and package verification are required parts of the release flow.

Replace `X.Y.Z` with a higher stable version. Preview first, then choose one
apply command (not both):

```bash
node scripts/release/bump.mjs X.Y.Z
node scripts/release/bump.mjs X.Y.Z --apply
# Alternative: apply and run strict maintenance/docs checks
node scripts/release/bump.mjs X.Y.Z --apply --verify
```

Writes require a clean working tree. The date defaults to today in UTC; use
`--date YYYY-MM-DD` for an explicit planned date. The helper coordinates
Cargo/lockfile, RPM/AppStream, current README/docs references, changelog, and
new release notes. Dependency pins, historical releases, and independent npm
package versions are preserved. Existing notes/tags or failed origin checks
block the bump. It never commits, pushes, tags, publishes, installs, or updates
dependencies.

Review the diff, curate notes/platform claims, and record fresh validation.
A bump is not proof of testing or publication. If verification fails, inspect
the uncommitted bump and rerun checks after fixing it; do not reapply the same
version. Security CI and manual acceptance are separate gates.

After validation, commit/push, annotate and push `vX.Y.Z` on the tested commit,
then manually dispatch **Linux Release Bundles** with that tag. Tagging alone
does not build packages. Inspect package metadata/startup and verify
`SHA256SUMS`, then publish a reviewed GitHub release with RPM/DEB/checksums.
Record platform/provider limitations and do not overwrite old notes, tags,
or assets. Do not restart an installed app during active file operations.

Dispatch the existing packaging workflow after the tested tag is pushed:

```bash
gh workflow run release-linux.yml --ref main -f tag=vX.Y.Z
gh run list --workflow release-linux.yml
# Use the completed successful run ID from the list:
gh run download RUN_ID -n browsey-linux-vX.Y.Z -D release-assets
cd release-assets
sha256sum --check SHA256SUMS
```

The workflow checks both package versions/dependencies and smoke-tests an
extracted DEB executable before producing the artifact. Inspect the downloaded
packages and record their exact checksums in the release notes. Complete clean
installation/upgrade checks separately for any distribution claims. Create a
draft release, upload both packages and `SHA256SUMS`, verify the uploaded assets,
and publish the reviewed draft. The packaging workflow does not publish a
release automatically.

Regression tests: `node --test scripts/release/bump.test.mjs`.

## Local maintenance reports and T3 follow-up

The optional local weekly reviewer only reads the original checkout and proposes
improvements. It never implements changes. Private reports live in
`docs/maintenance/agent-reports/`, locally excluded using `.git/info/exclude`.
This machine-specific folder is not part of the public repository.

`handling-log.json` records each report's SHA-256 identity and handling attempts,
with per-finding `implemented`, `rejected` or `deferred` decisions, reasons,
verification, remaining risks and an optional commit. The weekly reviewer reads
this history before suggesting work; it must recheck current code rather than
repeat implemented/rejected/deferred suggestions without new evidence.

The manually triggered **Process latest maintenance report** action in
[t3.json](../t3.json) runs:

```bash
node scripts/maintenance/process-report.mjs
```

This starts a separate Codex CLI job in the T3 terminal, not a turn in the current
chat. It checks the handling log first, skips completed reports, snapshots the
report, evaluates suggestions, implements appropriate small fixes and runs
relevant tests. Passed verification must match a successful captured command;
that evidence does not itself certify test relevance or native acceptance.
Intentional pre-fix regression failures are recorded as `expected-failure` only
with earlier failed-command evidence and a later successful rerun of the same
command. Unexpected historical failures that were corrected use
`resolved-failure` with the same evidence requirements; they are not silently
discarded or mislabelled as expected regressions. `failed` means unresolved and
still prevents an implemented result. Keep unperformed native checks explicit.
Results are retained in the private handling log. The action uses workspace-write
with network disabled, no inherited user config/rules/integrations and a bounded
runtime. Both the weekly reviewer and manual processor explicitly select
`--model gpt-6.1-sol` and `model_reasoning_effort="high"`. The action never commits,
pushes, installs or publishes. Review and commit its diff separately; record
that commit in the handling entry if desired.

For an unhandled report, use the original clean `main` checkout, installed Codex
CLI/ChatGPT login and existing test dependencies. Do not edit the checkout while
the action runs. The local service config supplies the original repository,
shared lock/state path, Codex binary and time limit when present. A different
worktree/clone is refused rather than silently editing the original repository.

```bash
# Read-only status; does not launch a model or edit files
node scripts/maintenance/process-report.mjs --check
# Only after inspecting an interrupted/failed attempt and resolving its changes
node scripts/maintenance/process-report.mjs --retry
node --test scripts/maintenance/process-report.test.mjs
```

Failures/timeouts retain changes and a `needs-review` attempt; they are not
automatically retried or marked completed. Completed deferred decisions are
reviewed outcomes, not finished implementations. Report archives and handling
history are not subject to the 12-week technical-log cleanup.

In T3 Code, choose **Add Action**, use the name above and paste the command into
**Command**, leave automatic worktree execution off, then **Save action**. If T3
offers import from `t3.json`, import the existing entry instead of adding a
duplicate. Do not run another modifying action/agent on the same checkout at the
same time. Keep personal reports/handling logs private; check Git exclusions
before publishing.

## Project layout and architecture

- `src/`: Rust/Tauri commands, metadata, watchers, keymap and persistence.
- `frontend/`: Svelte/TypeScript explorer, Settings and shared UI.
- `docs/`: user/developer guides, operations, audits and active/archived work.
- `docs-site/`: separately built/deployed documentation app.
- `packaging/`: desktop metadata and manual RPM packaging assets.
- `scripts/`: build, dev, install, maintenance, docs and release helpers.
- `tests/`: shared fixtures, native regression crates and integration helpers;
  module-local Rust and frontend tests remain alongside their implementations.
- `resources/`: bundled icons, schemas and native libraries, with provenance.
- `capabilities/`: Tauri permissions.
- `vendor/`: patched third-party sources; keep their licenses and upgrade checks.

The root keeps Cargo/Tauri manifests, `build.rs`, README, changelog, license and
third-party notices, plus tool configuration (`.cargo/`, `.github/`, `.semgrep/`
and `t3.json`). Architecture policies live under `docs/architecture/`.
Local build outputs such as `target/`, `dist/` and `gen/` are ignored, not source
folders. There is no active COPR configuration; `packaging/rpm/` retains an
optional manual rpmbuild spec, separate from the Tauri release workflow.

State uses SQLite (settings/bookmarks/stars/recents), a thumbnail disk cache,
and separate undo/log directories in the user data path. Import boundaries
and placement rules are in [Architecture imports](architecture/imports.md)
and [Architecture naming](architecture/naming.md). Module-level behavior is
documented on the docs site; operational policies and TODOs are indexed in
the project docs.

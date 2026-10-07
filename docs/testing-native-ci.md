# Isolated Linux native CI smoke

Scope: NT7-4 defines a manually dispatched, local-only real WebKitGTK smoke.
It does not provision or enroll this personal workstation as a runner. Dedicated
CI execution is NOT_RUN until an approved disposable image/runner is provisioned.
The local equivalent uses the already approved root and desktop isolation; its
run record is linked from [NT7 evidence](operations/linux-release/runs/2026-10-07-native-repeatability.md).

## Runner and image contract

[The workflow](../.github/workflows/native-smoke.yml) runs only manually selected
`main` on labels `self-hosted`, `Linux`, `X64`, `browsey-native-ephemeral`. Provision
a new VM/user/workspace/temp directory for exactly one job and destroy that VM
only through the approved infrastructure retention policy. The label or environment
marker alone is not proof of disposal; the provisioning operator must verify the
image and single-job runner configuration. Do not label a personal/shared desktop.
GitHub documents [runner labels](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/use-in-a-workflow)
and [ephemeral runner behavior](https://docs.github.com/en/actions/reference/runners/self-hosted-runners).
No PR/fork trigger, provider secrets, write token, repository publishing or normal
Browsey installation. Checkout does not persist credentials. Build dependency
network access is separate from the network-denied application smoke namespace.

Prepare GTK3/WebKitGTK 4.1 headers and runtime, the exact matching-engine
WebKitWebDriver, Rust/Cargo, Node/npm, Python with GI/AT-SPI, authenticated Xvfb,
D-Bus/GDBus, bubblewrap, xauth, xprop, setxkbmap, XTest libraries, fonts/fontconfig
and HTTPS trust paths required by the desktop contract. User/mount/PID namespace
creation must work unprivileged. No fallback to a personal display, mock/browser
preview, disabled sandbox or compile-only success if a prerequisite fails.

The operator seals a root-owned read-only `/opt/browsey-native/toolchain.json`: schema
1, imageId, arch, GTK version, app WebKit version, driver WebKit engine version and
SHA-256 for tauri-driver/WebKitWebDriver/Xvfb/bwrap. Match the engine version and
actual GTK/pkg-config versions, verify signed source/package provenance while
provisioning, and verify executable hashes before any fixture/profile write.
Native tools are supplied in `/opt/browsey-native/bin` and copied only into the
checkout's ignored `target/native-tools/bin`. The
[example manifest](../frontend/e2e-native/ci-toolchain.example.json) records the
locally accepted GTK 3.24.52 / WebKit 2.52.6 tool set; it is not a published image
or a CI execution receipt. A different approved image needs its own manifest
and native acceptance; matching numeric metadata alone is not acceptance.

Provision `BROWSEY_NATIVE_CI_EPHEMERAL=yes` in that disposable image. The workflow
passes the explicit approved image ID for comparison with the sealed manifest.
Provide a private mode-700, current-user-owned `RUNNER_TEMP` with a short absolute
path (for example `/tmp/bci`) and no inherited desktop/bus/SSH-agent variables.
The helper validates the full Unix socket path before writes. Existing credential
config, root, profile or retention registry stops preparation; no adoption/retry.

## Execution and evidence

1. Stage provisioned tools and run `node frontend/e2e-native/ci.mjs --prepare`.
   It exclusively creates the one approved ephemeral `ai_agent_testfolder` and
   mode-600 local-only config. Cloud credentials/configuration are absent.
2. Install locked frontend dependencies, run native policy/lint, build the separate
   `native-test` candidate with `scripts/dev/test-native-linux.sh --build`.
3. Run the existing `isolated.mjs --suite smoke --targets local --a11y` bodies on
   the private screen/bus with mount/PID isolation, real WebDriver/AT-SPI, independent
   generated-file checks and exact owned teardown/strict retention.
4. `ci.mjs --summary` requires exactly one registered run and exports only
   allowlisted IDs, hashes, versions and statuses. Missing cases/parts or teardown
   cannot be promoted to PASS. Upload only `ci-redacted.json` for seven days.

Raw reports, logs, config, screenshots and generated/recovery files are not uploaded
as public artifacts. Failed native runs preserve their evidence; cancellation/job
timeout without confirmed teardown is not a pass, and VM reclamation is governed
by the infrastructure retention policy. The CI entry point does not delete it.

Local `ci.mjs --check` is read-only version/hash preflight and cannot prepare CI
on an ordinary host. Provider/device/lifecycle tests remain explicit opt-in runs
on separately approved hosts/roots with their existing longer cloud time budgets.
Neither compilation nor local smoke certifies USB, network, OneDrive or mobile.

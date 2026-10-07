# Isolated native desktop scope

Prepared and explicitly approved by the maintainer for NT6, 2026-10-06.
Broader desktop services stay disabled outside this exact isolated scope.
Existing owned-folder Browsey operations remain
separate from that approval. No lifecycle/connect/mount/device action is included.

## Reviewable scope

Run a fresh generated UUID on a dedicated authenticated Xvfb screen, private
D-Bus, PID/IPC/user/mount namespaces and private profile. The harness namespace
exposes read-only system/code dependencies and the approved test directory.
Each GUI application gets a second filesystem boundary exposing only its exact
owned UUID run for writes. Personal HOME, desktop sockets, associations and trash
are absent from this mount tree. Network is isolated except explicit OneDrive
archive, cloud-export and cloud-trash cases using the copied private provider configuration.
The 2026-10-07 daily-driver follow-up reuses the approved isolated Nautilus scope
for two generated OneDrive files: prepare copies, cancel a drag, receive an
actual copy and independently verify cloud originals, retained staging and
receiver bytes. It enables only `prepare_cloud_external_copy` for that dedicated
mode; external launch IPC, personal paths and ordinary-session exceptions stay
disabled. The requested daily-driver tests authorize this bounded follow-up.

The same follow-up authorizes `cloud-trash`: normal Browsey upload and trash of
one generated `browsey-web-restore-<run UUID>.txt` in that run's exact cloud
subfolder. Only `trash_cloud_entries` for this one path is enabled; roots,
directories, other runs, extra inputs and ordinary modes are rejected. The
maintainer restores that exact file in the provider's web recycle bin. A finite
ten-minute read-only wait and independent original-path/size/SHA-256 checks
record restoration. No web account inventory, restore-all or bin purge is used.
The shared web session stays outside the isolated native desktop; the test does
not claim to automate or isolate the provider's web UI.

The prepared wrapper is `frontend/e2e-native/isolated.mjs`; namespace bootstrap
and service lifecycle are in `desktop-bootstrap.mjs` and `desktop.mjs`. Proof
requires distinct host/test namespaces, unavailable personal config, an actual
private bus and an Xvfb client rejected without the private Xauthority cookie.
Exact owned PID/start/profile identities still gate UI control and teardown.
The namespace supervisor must exit after the test, eliminating service children.

The separate approval covers only:

- One isolated Nautilus fixture window on the private display/bus, exchanging
  generated files/folders with owned Browsey windows. X11 receiver capabilities
  are recorded; copy results cannot certify Wayland move or portal behavior.
- System clipboard round trips on this dedicated screen, with generated owned
  paths only. The existing empty/no-op clipboard overrides remain the default
  for ordinary native sessions.
- Trash, restore and purge of generated owned files in the isolated filesystem,
  private trash catalog and profile. No physical service or personal trash is
  visible. Validate every original path and private identifier before dispatch.
- Open With using generated dummy handlers, controlled failures, spaced names
  and the default-program checkbox against private MIME/application state.
  Validate the selected handler before enabling its launch.

Use the current finite retention limits, small fixtures and one-use scoped
exceptions. Unknown modes, programs, paths and shared-desktop execution fail
closed. Mock IPC or delivered pointer actions cannot substitute for independent
native file effects. This specification does not declare these cases accepted.

## Prepared font-cache recovery action

Blocked startup `d8024666-22b6-4eba-b0df-702155929e50` retains its actual report
and all original data. GTK/fontconfig created a mode-755 cache directory with
36 relative symbolic references to 12 generated cache files in the same directory,
plus a generated cache tag, causing the strict no-link/private retention audit
to reject it. No reference has been followed during review.

A mode-600 tar archive and SHA-256/index were prepared inside that exact run's
private artifacts. Read-only archive validation confirms all link spellings and
directory metadata and all 13 regular files, without extraction or following
links. The archive is 1,699,840 bytes, with SHA-256
`c06536ffc2a95a54bb2ce9d898929bcd9a2a570be91b6c02103db3234aa1637e`.
The proposed action
is to revalidate identity/hash, replace only that generated font-cache directory
with its verified retained archive, then run the explicit retention review.
No test file, report, credential or undo/recovery payload would be removed; the
original BLOCKED result remains unchanged. The maintainer separately approved
this exact replacement on 2026-10-06; all source/cache identities and archived
file hashes were revalidated before removing only the archived cache nodes.
Fresh desktop profiles use controlled font configuration and read-only system
font caches. The accepted run's strict private retention audit passes without
altering any retained permissions or removing render cache data.

AT-SPI uses its supported `ATSPI_DBUS_IMPLEMENTATION=dbus-daemon` selection
([upstream launcher](https://github.com/GNOME/at-spi2-core/blob/main/bus/at-spi-bus-launcher.c)),
set before launching the private session bus, avoiding the host journal dependency.

The local NT7 `smoke` suite reuses the same isolated desktop and ordinary native
session restrictions. It enables no desktop service exception or network.

The NT7 `repeatability` suite has the same local-only ordinary restrictions; its
one restart retains only the already exclusive profile and requires confirmed
old app/driver teardown. It does not reuse historical run directories.

The local NT7 `measurements` and `cancellation` suites use ordinary scoped IPC
and exact generated transfer probes on the private desktop. They enable no
external desktop service, personal account, network or lifecycle transition.

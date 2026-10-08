# Native file drag regression checks

Ordinary drag exports local files without Alt. Without a start modifier, Browsey
advertises copy and move; the receiver chooses the action and performs the operation.
Ctrl/Meta held at start limits the offer to copy; Shift limits it to move. That
explicit action also remains fixed for internal drops, keeping the cursor/offer
and operation consistent. Otherwise internal drops still use live modifiers.
Browsey does not delete sources on drag completion. On Linux, cloud selections
can also be copied directly to another Browsey instance with matching source
rclone configuration. Other applications still require a prepared local copy.

## Window teardown crash

The September 2026 Linux SIGABRT occurred when GTK destroyed a drag callback
during Tauri window cleanup. The former plugin callback owned an IPC `Channel`;
dropping that channel evaluated JavaScript and re-entered the runtime's already
mutably borrowed window registry. The callback need not have been executing, and
the trace does not prove that the user closed the window during an active drag.

`src/native_drag.rs` hooks the existing WebKit source drag instead of starting a
second native drag. Its GTK callback captures neither Tauri handles nor channels.
The former `drag` dependency and asynchronous start command are removed entirely.

## URI list interoperability

WebKitGTK sanitizes a DOM `text/uri-list` as a single URL, stripping line breaks
and concatenating multiple filenames. This was reproduced with a real GTK URI
receiver, not inferred from browser mocks. See WebKit's
[DataTransfer implementation](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/dom/DataTransfer.cpp)
and [GTK drag source](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/API/gtk/DragSourceGtk3.cpp).

The frontend therefore writes one URL-encoded envelope on Linux. An AFTER
`drag-data-get` handler replaces the borrowed GTK selection with separate, escaped
`file:` URIs before delivery. Running after WebKit is essential: WebKit can install
its handler lazily, and an ordinary handler can run before the data is populated.
No JavaScript evaluation runs in this callback. The private
envelope is never intended as a URI to open. Invalid envelopes produce an empty
selection; relative paths, cloud paths, empty selections and NUL are rejected.

For portal targets, the bridge registers files with `autostop=false`, rather than
using GTK3's `set_uris` portal conversion (which defaults to stopping after the
first read). Nautilus/GTK4 can retrieve the same token more than once; the former
conversion caused `AccessDenied: Invalid transfer` on the second read. One token
is reused per drag and `StopTransfer` is queued at drag end (including cancellation),
the next drag start, or callback destruction. Cleanup captures no Tauri handles.
Registration opens metadata-only `O_PATH` descriptors in batches of 16 and uses
bounded D-Bus calls; no file contents are read. Errors leave an empty selection
and are logged. Plain URI-list destinations do not need the portal.

The protocol is documented in the upstream
[FileTransfer interface](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.FileTransfer.xml).

Returning native self-drops retain the internal source snapshot/modifiers and
use the existing transfer/conflict workflow exactly once. Incoming external drops
remain copy-only. Other platform webviews receive standard URI lists but are not
covered by the Linux native test.

## Cloud copies between instances

Cloud drags use a single opaque `browsey-drag://cloud/<token>` offer, preserving
original Drive object IDs in private metadata. Registration and account checks
run on blocking workers. Hover resolves the selection once; drop verifies the
source configuration afresh before the existing copy/conflict workflow. Shift
cannot turn an incoming cloud copy into a move, and Wastebasket rejects it.
Normal local drags do not register cloud metadata or execute rclone.

The source and receiver must share a private runtime/cache namespace and matching
source rclone configuration, including remote name, account credentials and root.
Access-token refresh is ignored; separately authenticated configurations can be
rejected even when they access the same account. References expire after two
minutes; ordinary drag completion releases them after a short delivery grace.
The GTK bridge passes these offers through without exporting portal files or
adding Tauri handles/channels to GTK callbacks.

The explicit isolated native suite uses a test-only rclone configuration and two
approved cloud roots named `Onedrive` and `Google Disk`. Add `cloudPeer` to the
usual local/cloud config, pointing to the second
`rclone://Remote/ai_agent_testfolder`, then run:

```sh
node frontend/e2e-native/isolated.mjs --suite cloud-drag --targets local,cloud --config frontend/e2e-native/config.cloud-drag.local.json
```

Only this suite admits the second cloud target and reference commands. Its private
Drive ID catalog binds independently listed IDs to generated paths under the
exact owned run; spelling an owned-looking name cannot authorize another ID.
The source and receiver run as separate Browsey processes on an isolated desktop.

The 2026-10-08 native run passed OneDrive → Google Drive and Google Drive →
OneDrive (including Shift), files, nested folders, empty folders, Unicode/reserved
names, cancelled drag and cancelled name conflict. Independent readback verified
source preservation and destination bytes. The separate local native regression
also passed two-window copies, a real Nautilus copy and active-drag teardown.
This does not establish direct cloud dragging into Nautilus.

## Automated checks

Rust tests cover URI encoding/validation; frontend tests cover export payloads,
cloud/mixed rejection, ordinary drag, and returning native self-drops. Those
tests alone cannot detect WebKit sanitization or GTK teardown crashes.

Frontend regressions also omit source `dragend` deliberately: feedback must hide
on window exit, fresh pointer presses/releases must clear abandoned state, and a
different incoming native offer after exit must remain copy-only. Returning
self-drops keep their source/action, while accepted asynchronous transfers hide
feedback without being cancelled or repeated. These mocked checks do not establish native WebKit
acceptance for the missing-`dragend` sequence; verify it in the installed app by
dragging outside, cancelling or dropping, then returning and trying another drop.

The scoped NT6 X11 candidate also reproduces DOM `dragend` before Tauri's matching
native drop. Successful same-window completion now retains its exact paths,
destination, point and action for up to two seconds, consumed once. Cancellation,
Escape, a fresh press, expiry or a mismatched offer cannot reuse the action.
The [NT6 record](operations/linux-release/runs/2026-10-07-native-desktop-interaction.md)
contains independently verified native file/folder moves after the fix, plus
owned two-window copies, Nautilus copies and active-drag teardown.

Zero-button mouse/pointer motion must not end an internal drag or convert its next
native hover into an external offer. Targeted frontend regressions verify that
subsequent DOM `dragover` coordinates still position the label and that a native
self-drop retains the explicit start action. These checks do not establish the
actual native event cadence or smooth tracking in the installed WebKit build.

Row/card `dragleave` with a null `relatedTarget` must not unmount the feedback
label. Native leave/blur still hides it on webview exit, and root DOM leave is
handled separately. Listing consumers subscribe to scalar target/count/active
values: moving over rejected files must not republish these values, resolve a
destination, or rebuild Explorer prop bags. Unit tests cover these invariants;
a large mocked list/grid checks that the same label node survives row changes.
Repeated movement over the same valid background destination must also retain
its target and highlight without republishing listing inputs. This applies to
DOM hovers, returning internal native offers, and external native offers. Drop
guards still run for every hover: opening a dialog immediately removes acceptance.
Tests cover stable target/action notifications, DOM attributes and resolver calls.
The disposable browser comparison measured about 11 ms per rejected-file hover
before this isolation and 0.6 ms after, with 200 and 100,000 entries; these are
development JavaScript-update measurements, not installed WebKit timings or
proof that the reported native lag is resolved.
An additional comparison with 10,000 mocked files measured about 10–16 ms per
same-background hover before retaining the target, versus 0.2–0.3 ms after. The
first change into a new destination still updates the listing; further motion
over that destination does not. These are synthetic dispatch/update timings in
the development Chromium preview, not native input-to-paint measurements.

In a real Linux graphical session with X11/XWayland available, run the explicit
native regression separately:

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo test webkit_file_export_and_window_teardown -- --ignored --test-threads=1 --nocapture
```

This opens a Tauri/WebKit source window and GTK receiver. Python3 and libXtst
simulate an actual mouse drag between these test windows, so do not move the
pointer while it runs. The receiver checks a two-file selection with spaces,
Unicode, reserved URL characters and a newline, then closes the source through
Tauri. Fixture paths are protocol-only; no source files are read or changed.
The test does not run Browsey's database setup or device monitors. Use an external
timeout. X11/XWayland is needed for input automation, not for production drag.

Rust Quality also runs this regression on an isolated Xvfb screen. On X11 the
fixture places source and receiver side by side, so it does not need a window
manager. The virtual-screen pointer does not affect a real desktop session.

For real file operations against an isolated Nautilus on Omarchy/Hyprland, use
the opt-in Wayland acceptance modes (one at a time):

```sh
GDK_BACKEND=wayland BROWSEY_TEST_NAUTILUS_BACKEND=wayland BROWSEY_TEST_NAUTILUS_MODE=move WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo test webkit_file_export_and_window_teardown -- --ignored --test-threads=1 --nocapture
```

Repeat with `BROWSEY_TEST_NAUTILUS_MODE=default` and `copy`. This requires Python3,
Nautilus, `dbus-run-session`, `hyprctl`, and already-granted access to `/dev/uinput`.
The fixture explicitly advertises the requested action; frontend unit tests
independently cover modifier-to-action mapping. This avoids depending on synthetic
keyboard state crossing toolkits. The helper creates a temporary input device, drives only the test windows and
destroys the device afterwards. Do not use the keyboard/mouse while it runs.
It creates its own temporary files, starts Nautilus on a private session bus with
isolated data/config/cache/runtime directories, checks filenames/content and source state,
then removes only its fixtures and stops only its own process group. No device
permissions or desktop configuration are changed. Prefer an external 60s timeout.

Also run all three modes with `BROWSEY_TEST_NAUTILUS_BUS=session`. This keeps
Nautilus and Browsey on the real shared session bus, exercising portal negotiation
that the private-bus test misses. Close Nautilus first: the helper refuses to run
if its D-Bus name is already owned. Data/config/cache/runtime remain isolated,
and only disposable fixtures are used. This requires `gdbus` and a running
FileTransfer portal. Do not start Nautilus separately while the test runs.

The portal lifetime test needs no pointer automation:

```sh
cargo test portal_token_survives_multiple_reads_and_expires_on_drop -- --ignored --test-threads=1 --nocapture
```

It registers 18 disposable entries (including a directory), reads the same token
three times, then verifies that dropping the transfer invalidates it. It also
checks registration failure for a missing file.

Nautilus 50.3.1 on X11 explicitly forces external-process drops to COPY in
[`on_view_drop` / `on_item_drop`](https://gitlab.gnome.org/GNOME/nautilus/-/blob/50.3.1/src/nautilus-list-base.c).
Consequently the X11 URI receiver/copy tests cannot certify real move semantics.
The Wayland acceptance modes use real compositor input, not XTest events confined
to XWayland. Never compensate for a receiver's COPY by deleting sources yourself.

## Manual acceptance

Use disposable local fixtures and a separate destination folder:

1. Drag a file and a multiple-file selection to Files/Nautilus without modifiers.
   Verify contents and source/destination state against the negotiated action.
2. Repeat holding Ctrl **before drag start** (copy: originals remain), then Shift
   before drag start (move: originals relocate). The explicit choice stays fixed
   until the gesture ends; cancel and restart to change it.
3. Cancel a drag with Escape, then close Browsey. Originals must remain.
4. Complete a drop, then close Browsey. Repeat several times.
5. Close Browsey with an active native drag, where the desktop permits it.
6. Verify ordinary internal drag/drop and incoming external drops still work.
7. Check the journal/coredump list for new Browsey crashes.

The default native test checks WebKit-to-GTK URI exchange and window teardown.
The optional Nautilus modes check real file operations. Neither covers every
compositor/backend or all manual acceptance cases above.

Verified locally on 2026-09-26 with Nautilus 50.3.1 and WebKitGTK 2.52.6:
the GTK URI receiver accepted both exact paths, and real Wayland Nautilus runs
passed `default -> move`, `copy -> copy`, and `move -> move`, checking both file
contents and source state before closing the Tauri source window. Keyboard-to-
offer mapping and returning self-drops are covered separately by frontend tests.
After reproducing the portal expiry bug, all three modes were also verified on
the real shared session bus with the explicit token-lifetime fix. The standalone
portal test verified repeated retrieval and rejection after cleanup.
The acceptance helper depends on an undisturbed graphical session; unsuccessful
synthetic-input attempts during development are not treated as successful tests.

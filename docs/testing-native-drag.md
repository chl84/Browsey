# Native file drag regression checks

Ordinary drag exports local files without Alt. Without a start modifier, Browsey
advertises copy and move; the receiver chooses the action and performs the operation.
Ctrl/Meta held at start limits the offer to copy; Shift limits it to move. That
explicit action also remains fixed for internal drops, keeping the cursor/offer
and operation consistent. Otherwise internal drops still use live modifiers.
Browsey does not delete sources on drag completion. Cloud selections remain
internal-only until downloaded to a local folder.

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
No filesystem I/O or JavaScript evaluation runs in this callback. The private
envelope is never intended as a URI to open. Invalid envelopes produce an empty
selection; relative paths, cloud paths, empty selections and NUL are rejected.

Returning native self-drops retain the internal source snapshot/modifiers and
use the existing transfer/conflict workflow exactly once. Incoming external drops
remain copy-only. Other platform webviews receive standard URI lists but are not
covered by the Linux native test.

## Automated checks

Rust tests cover URI encoding/validation; frontend tests cover export payloads,
cloud/mixed rejection, ordinary drag, and returning native self-drops. Those
tests alone cannot detect WebKit sanitization or GTK teardown crashes.

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
The acceptance helper depends on an undisturbed graphical session; unsuccessful
synthetic-input attempts during development are not treated as successful tests.

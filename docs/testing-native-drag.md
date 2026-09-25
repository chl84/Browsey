# Native file drag regression checks

Alt-drag exports local files in copy mode. The receiving application performs the
copy; Browsey does not remove the source or report transfer completion. Cloud
selections must be downloaded first.

## Window teardown crash

The September 2026 Linux SIGABRT occurred when GTK destroyed a drag callback
during Tauri window cleanup. The former plugin callback owned an IPC `Channel`;
dropping that channel evaluated JavaScript and re-entered the runtime's already
mutably borrowed window registry. The callback need not have been executing, and
the trace does not prove that the user closed the window during an active drag.

`src/commands/native_drag.rs` uses the same pinned `drag` backend directly, with
a capture-free function pointer instead of the unused JavaScript channel. Keep
that callback free of Tauri handles and IPC even if completion feedback is added
later. The command returns startup status, not copy completion. GTK setup runs on
the GUI thread; source validation runs on a blocking worker.

## Automated checks

Ordinary Rust tests cover source validation; frontend tests cover command payload,
cloud/mixed rejection, and startup failures. These tests alone cannot detect GTK
teardown crashes.

In a real Linux graphical session with X11/XWayland available, run the explicit
native regression separately:

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo test native_drag_window_teardown -- --ignored --test-threads=1 --nocapture
```

This briefly opens a dedicated blank Tauri window, starts a real GTK file drag,
then closes the window through Tauri. It offers the repository's `Cargo.toml` as
a read-only copy source; it does not automate a drop or modify/delete the source.
It does not run normal Browsey initialization, USB monitoring, or database setup.
Use an external timeout if automating this in a graphical test environment.
X11/XWayland is used for repeatability: this test starts drag programmatically,
without a real pointer-button event. Wayland may reject that synthetic startup;
that is a startup error, not evidence of the former teardown abort. Check actual
Wayland gestures manually instead of treating every startup refusal as a crash.

## Manual acceptance

Use disposable local fixtures and a separate destination folder:

1. Alt-drag a file and a multiple-file selection to Files/Nautilus; verify copied
   contents and that every original remains.
2. Cancel an Alt-drag with Escape, then close Browsey.
3. Complete a drop, then close Browsey. Repeat several times.
4. Close Browsey with an active native drag, where the desktop permits it.
5. Verify ordinary internal drag/drop and incoming external drops still work.
6. Check the journal/coredump list for new Browsey crashes.

The native automated test covers startup/teardown, not acceptance of a drop by
another file manager, cancellation gestures, or every compositor/backend.

On 2026-09-25 the native teardown regression passed three consecutive runs under
XWayland in the local Omarchy session. The default Wayland backend also passed
three runs, but a fourth was refused at drag startup as described above. Real
cross-application drop and cancel gestures still require the manual checks above.

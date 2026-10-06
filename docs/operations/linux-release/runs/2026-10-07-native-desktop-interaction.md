# Native desktop interaction

Date: 2026-10-07 (Europe/Oslo). Scope: NT6-1 through NT6-8 in the
[native suite TODO](../../../todo/TODO_NATIVE_TEST_SUITE.md). Each completed
part receives its own commit. Final production build and push follow completion
of all eight parts; no installed Browsey instance is changed.

## NT6-1: owned native drag/drop

Run `6d5f0da5-8762-47ab-8d0c-17904cb9466a` is native PASS: 11 declared parts
plus AT-SPI accessibility. The real WebKitGTK candidate executes file/folder
copy and move, default same-filesystem move, multiple files, a mixed grid
selection, Unicode/reserved/spaced names, self-drop rejection, Escape cancellation,
three incoming two-window routes, two Nautilus copy routes and shutdown/restart
with a real active native drag. Independent exact trees, kinds and bytes verify
the source and destination; no synthetic drag event or file-operation IPC replay
stands in for acceptance.

The maintainer separately approved the
[desktop isolation contract](../../../testing-native-desktop.md). The private
Xvfb cookie rejects unauthenticated clients; mount/PID namespaces differ from
the host, personal desktop config is absent, and the session bus is private.
Each application can write only its exact generated UUID and namespace-local
temporary data. Captured PID/start/executable/profile/run identities gate control
and teardown of both Browsey windows and the one Nautilus fixture window.
All recorded app/driver teardown steps pass; the outside namespace supervisor
exits, terminating activated service children. Strict private retention passes
(167 entries, 13,347,504 bytes), preserving all generated/recovery data.

Incoming Browsey drops are copy-only, including a source gesture using Shift.
Nautilus 50.3.1 on X11 also enforces external-process copy. These checks certify
actual copies and source preservation, not external Wayland moves or portal
negotiation. Accepted input applies modifiers after selection press and before
pointer movement. A separate pre-press Shift attempt produced no trusted source
drag and remains BLOCKED; it cannot certify that input sequence. One US keyboard
layout is sufficient for this run.

Earlier run `e8f404ac-cd61-4317-96b9-e4fd42ee7208` reproduces a real move defect:
DOM dragend precedes the native drop, clearing the internal action and causing
an incoming-copy fallback. Browsey now retains a successful completion's exact
paths, destination, point and action for at most two seconds, consumes it once,
and rejects mismatched, expired, cancelled or newly started offers. It hides
feedback immediately and still lets teardown clear the state. The native rerun
verifies source removal for both file and folder moves. Eight focused regression
cases cover late completion and unsafe action reuse; all 60 drag-policy unit
tests pass. Unit tests supplement, rather than replace, the actual native result.

Failed startup reports remain retained. Run
`d8024666-22b6-4eba-b0df-702155929e50` stays BLOCKED: its font cache was preserved
in an independently verified private archive before the maintainer-approved
exact replacement and explicit retention review. Fresh font configuration uses
read-only system caches; no retained permission was silently repaired. Finite
retention limits were explicitly raised to 128 runs / 896 MiB for NT6 capacity,
with the existing 128 MiB per-run and traversal/time limits unchanged.

Remaining NT6 parts are not accepted by this record yet.

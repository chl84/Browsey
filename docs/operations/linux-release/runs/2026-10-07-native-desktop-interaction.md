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

## NT6-2: native feedback under listing load

Run `610329a6-50e5-4502-8811-c8bef18c7e83` is native PASS: four list/grid
cancel/copy parts plus accessibility, each with 120 generated filler files.
Actual XTest movement crosses at least ten visible rows/cards, real empty
listing space and a valid destination. Trusted DOM/Tauri events and animation
frame samples show multiple actual label positions, a stable feedback DOM node,
rejected-file labels recovering to Copy and no stuck feedback after release.
Exact independent source/destination trees prove cancelled work preserved every
file and accepted drops created exactly one source copy. Native task/listener
resources release. All four teardown steps, outside namespace exit and strict
private retention pass (541 entries, 1,163,600 bytes).

Timings/positions are descriptive host-specific observations, not a synthetic
render count or an invented latency budget. An earlier observer-only failure
(`97d81e4f-3551-4f0b-b1d7-57ca811609bf`) accessed the null native leave payload;
the read-only observer was corrected and the failed report retained. No product
change was needed for the accepted feedback scope.

| Part | Native/DOM events | Visible label frames | Distinct label positions |
| --- | ---: | ---: | ---: |
| List/cancel | 77 | 130 | 38 |
| List/copy | 103 | 177 | 50 |
| Grid/cancel | 89 | 148 | 44 |
| Grid/copy | 115 | 202 | 56 |

Every visible sampled frame matched its most recent actual input point plus
the existing 12-pixel label offset on this screen. This checks native tracking;
it does not measure input-device or compositor latency.

## NT6-3: isolated clipboard and private trash

Run `91ed4148-1bb1-44c5-9353-d6425ad9a73c` is native PASS: eight parts plus
AT-SPI. Two actual Browsey processes exchange two generated files including a
Unicode/spaced/URI-special name. Independent X selection reads agree with the
real backend's paths and mode. Cut removes both sources and clears the actual
selection; copy preserves both sources. Private trash checks cover mixed
file/nonempty-folder trash, selected restore, purge Cancel/confirm and Empty
Wastebasket Cancel/confirm. Original paths, exact catalog identifiers and stored
bytes are independently checked; an unrelated sentinel is preserved throughout.
All main/peer teardown checks and namespace exit pass. Strict retention passes
(73 entries, 752,548 bytes). Personal trash and desktop services are absent.

Runs `745ea1d4-de2f-46f0-9394-5e059c5797ce` and
`c2be736b-4d86-4884-99ba-c8a717d34623` retain the actual cut-as-copy evidence.
The GNOME parser previously accepted a URI-list comment as a copy header,
losing cut mode (or the first path for plain URI lists). It now requires a real
copy/cut header and otherwise falls back to the URI-list parser. X11 clear uses
EOF input instead of waiting with its stdin pipe still open. Five clipboard
unit tests pass, including two meaningful parser regressions; native acceptance
also proves successful clearing and command completion.

Earlier run `5405507e-80c1-4f1e-92fd-412278365e60` recorded successful file
operations but its purge-warning assertion was too broad: the remote warning
matched while the same dialog incorrectly promised local Undo. That raw PASS
report is retained, but does not certify truthful purge wording. The strengthened
native check and final rerun verify the corrected dedicated Wastebasket warning;
four deletion-dialog unit tests pass. Native policy tests: 157 PASS.

The existing separately approved namespace/display/bus contract gates real
clipboard/trash dispatch; ordinary sessions retain disabled services. IDs and
original paths must belong to the exact private catalog/owned roots, with no
links. The local xclip dependency is the signature-verified Arch xclip 0.13-6
binary, extracted only into ignored test tools (SHA-256
`1a757a1ae88441c9fc6101c0750d86a1afb5cb7b2073a0ed98967c41dc292d20`);
no system package or personal clipboard was changed.

Remaining NT6 parts are not accepted by this record yet.

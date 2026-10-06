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

## NT6-4: keyboard, focus and accessible names

Run `132ade44-9475-464d-8376-14935ad1e485` is native PASS: eight parts plus
accessibility. Actual keys verify list/grid navigation, range selection and
Escape, complete forward/reverse modal Tab cycles, first-Escape text blur then
modal close, focus restoration, context-menu arrows/Home/Escape, Properties
names via dynamic AT-SPI snapshots, tooltip Escape and main-menu Escape during
an actual owned transfer. A declared five-second finalization hold keeps that
real transfer observable; independent trees and a consumed checkpoint prove
its result, and all task/listener resources release. All teardown, namespace
exit and strict retention pass (55 entries, 755,124 bytes).

Properties formerly exposed its entire tab strip as its accessible dialog name
(raw run `56e90e9b-320e-47cc-a64e-03374f6566f4`). ModalShell now accepts an
explicit accessible name for rich headers; Properties exposes exactly
“Properties” while retaining individually named tabs. The main menu now focuses
a control on opening, restores the opener before launching another dialog and
stops Escape propagation so closing is requested once. Six focused unit tests
pass; lint and frontend type checking pass.

Intermediate slow-operation assertions wrongly assumed the menu remains open
after changing view. Topbar intentionally closes it; those BLOCKED reports
remain preserved, and their assertions do not establish a focus-stealing defect.
The final check follows the actual policy: focused Escape closes/restores the
menu, and selecting a view closes it while the transfer continues. No view-switch
focus policy was changed. One US keyboard layout is the accepted input scope.

## NT6-5: private appearance, zoom and thumbnails

Run `db4ffc74-3ad2-43a4-83c7-d423e320aba9` is native PASS: eight parts plus
accessibility. A cold private cache displays 48 generated PNGs; actual pending
valid-image states transition to decoded cache thumbnails without changing sorted
order. Corrupt PNG bytes and a generated QOI stream under a PNG name retain
usable fallback icons. Four real Settings combinations cover light/dark style
and cozy/compact density, with observed classes/palette and retained screenshots.
The XTest helper validates the exact app PID/executable/profile/window before
real Ctrl-wheel input: sizes 96→128→160→192→160→128→96 are independently read
from actual layout. No synthetic wheel event is used.

A later generated PNG and explicit refresh verify stable order, decoded thumbnail
and Properties name/type/independent byte size. Grid filter Reset restores every
entry. Progress and Settings bounds fit the native 1800×1000 viewport during an
actual declared slow transfer; it completes with exact preserved source and
copied bytes. All 51 image/fallback files retain exact binary digests. Task and
callback resources release. Teardown/namespace exit and strict retention pass
(278 entries, 11,883,252 bytes). Native policy tests: 157 PASS. No product change
was needed for this appearance scope; no global theme was read or changed.

Cold-grid completion was observed after 435 ms on this host with this small
workload; it is descriptive, not a regression budget. Raw thumbnail observations
remain in bounded private artifacts with SHA-256 references, keeping the report
below its existing one-MiB read limit. Earlier BLOCKED run
`f15ce777-d325-46b1-934a-7fa6ae8058b4` retains its larger original report and all
files/screenshots; the harness sent two Escape codes in one key sequence instead
of separate presses for text blur and modal close. The corrected real input
sequence passes in fresh runs. No report, permission or recovery data was altered.

## NT6-6: fixture-only actual watchers

Run `b099e54b-60a8-4c68-84a8-5206b91198c2` is native PASS: six parts plus
accessibility. The separately identified mode installs an actual notify watcher
on an explicit owned local directory. Real external create/edit/rename/delete
produce actual dir-changed events; UI membership and changed byte-size metadata
reconcile without F5. An external write during an actual five-second held copy
and UI Refresh preserve both the copy and the external file. A new queued change
followed by owned window/driver shutdown and one restart preserves all bytes;
the fresh process installs its own watcher and lists the exact resulting files.
Independent trees preserve the unrelated sentinel throughout. Both old and new
four-step teardown, namespace exit and strict retention pass (55 entries,
751,823 bytes); task/listener resources release.

The native-only watch branch runs before expand_path, home fallback, disk/GVFS
and bookmark discovery. Missing/relative/traversal/outside/cloud paths fail
closed, and watcher-start failure is returned rather than silently acknowledged.
Ordinary sessions retain disabled watchers. Twelve Rust native guard tests pass,
including denial before filesystem I/O; native policy tests: 157 PASS.

Retained BLOCKED run `5f26d6b5-6d7e-4af4-b2c4-26a5dd701634` passes its five
functional parts and proves old-process teardown, but immediate same-port driver
preflight reports EADDRINUSE. One deliberate restart now uses a fresh fixed,
preflighted port pair (4448/4449) after confirmed old teardown; no case, mutation
or launch is retried. Readiness and captured listener ownership use that pair.
A further failed readiness run remains retained. Failure reporting also preserves
the original startup error when no new driver/candidate identity exists, instead
of replacing it with an undefined-process inspection error. The final native run
proves the complete declared shutdown/restart scope; it does not certify old-port
reuse timing. No normal application watcher policy was changed.

## NT6-7: generated archives and actual cloud staging

Run `030ae108-db3f-4c16-89a9-065f493c1c9e` is native PASS: 22 parts plus
accessibility. Generated in-memory ZIP, TAR, TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZST,
7z, stored RAR4 and standalone GZ/BZ2/XZ/ZST payloads extract through actual
Browsey context menus. Independent exact tree/byte/digest checks preserve each
original archive. The application creates an AES-256 ZIP; wrong-password feedback
returns an empty password field, a correct entry completes extraction, and
password cancellation preserves the existing output. Compression-dialog Cancel,
unique extraction destinations protecting an existing sentinel, and a corrupt
ZIP mixed with a good archive verify cancellation and truthful 1-success/1-failure
batch feedback.

Hostile ZIP/TAR entries cannot write outside the destination or change the parent
sentinel. ZIP 8.6 safely normalizes an absolute entry to a relative name inside the
output and skips both traversal entries; TAR skips all three unsafe paths and one
symlink. The earlier raw FAIL `26e3cedc-2eca-4f98-bb9c-3ef8858abfec` expected
ZIP to skip the safely normalized name as well; that incorrect assertion was
corrected without changing product extraction behavior. Its original report and
all generated data remain retained.

Actual OneDrive compression downloads the generated source into the private
workspace and uploads the new ZIP; extraction downloads it, extracts locally and
uploads a fresh suffixed folder. Exact independent source/archive/output bytes
match. A repeated archive name reports an existing-destination error and preserves
both originals. The two compression stages and one extraction stage, including
all four materialized payload files, remain independently inspected and retained.
Six-hundred-second operation and 180-second UI waits accommodate cloud work.
No file operation is automatically retried.

Initial setup `a92da4d9-3380-4e19-8a97-1401990ee16a` is BLOCKED: system CA
bundle symlink referents were absent from the namespace. A bounded read-only
provider diagnostic confirmed certificate failure. Both isolation layers now
expose only the system CA trust directory read-only; private credentials and
personal desktop services retain their existing boundaries. The fresh complete
run proves connectivity and all declared effects. All four teardown stages,
namespace exit and strict retention pass (158 entries, 808,079 bytes); archive
progress callbacks and actual cancel tasks release.

Existing Rust decoder/security tests are reused: 44 extraction and 10 compression
tests pass, plus the new archive-scope denial test. Native policy: 157 PASS.
Native encrypted UI coverage is ZIP; encrypted 7z/RAR/codecs and resource caps
remain covered by the existing Rust tests, not new duplicated native bodies.
Large-archive cancellation, every encrypted codec/provider and concurrent hostile
writers are outside this bounded native acceptance. No product archive change
was required.

NT6-8 is not accepted by this record yet.

# Native case-to-acceptance index

Updated: 2026-10-07. This maps executable native case IDs to supporting rows
in the [daily-driver validation checklist](daily-driver-validation-checklist.md).
It does not close those broader rows or certify an installed production build.
The [NT7 run record](runs/2026-10-07-native-repeatability.md) records actual
accepted profiles, timings, CI-definition verification and final checks.

## Executable traceability

The shared suite registry is `frontend/e2e-native/catalog.mjs`;
`acceptance-map.mjs` supplies the explicit mapping below. Native reports include
each case's `acceptanceRows`, and repeated-profile cases retain `sourceCaseId`.
CI receipts include only allowlisted IDs, mappings, hashes, versions and statuses.
New unknown case families stop report generation instead of silently acquiring
acceptance. `run.mjs --plan --targets local --tier smoke` previews declared IDs
and row mappings without target inspection or writes.

The policy test checks all 47 executable suites against synthetic
provider configurations and all-pairs declarations, including report requirements,
parts and tier membership. This is catalogue/policy verification, not execution
of all those cases or a new five-provider native pass.

| Case family | Case-ID pattern | Supporting rows |
| --- | --- | --- |
| desktop-drag | `^desktop-drag$` | A1-1, A2-1, A2-6 |
| desktop-feedback | `^desktop-feedback$` | A1-1, A2-3, A2-5 |
| desktop-services | `^desktop-services$` | A0-2, A1-2 |
| desktop-keyboard | `^desktop-keyboard$` | A2-1, A2-2, A2-7, A3-1 |
| desktop-appearance | `^desktop-appearance$` | A2-3, A2-4, A3-2 |
| desktop-watchers | `^desktop-watchers$` | A0-7, A2-1, A2-6 |
| desktop-archives | `^desktop-archives-(local\|cloud)$` | A0-2, A0-3, A2-2, A2-5 |
| desktop-open-with | `^desktop-open-with$` | A1-3, A2-2 |
| accessibility | `^accessibility-local$` | A3-1 |
| owned lifecycle | `^(lifecycle-(owned-window\|profile-reuse)\|interruption-local)$` | A0-4, A2-6 |
| undo/history | `^(undo-copy-local\|history-local)(-(fresh\|reused))?$` | A0-5 |
| active writes | `^races-` | A0-7, A0-3 |
| cancel/failure/overwrite | `^(cancel\|moves\|iofaults\|overwrite)-` | A0-2, A0-3, A2-4, A2-5 |
| partial batches | `^batch-` | A0-2, A0-3, A2-5 |
| byte progress | `^progress-` | A2-4, A2-5 |
| OneDrive | `^provider-cloud(-errors)?$` | A0-2, A4-1, A4-2, A4-3, A4-4 |
| MTP | `^provider-mobile$` | A1-5, A4-1, A4-4 |
| network | `^provider-network$` | A0-2, A1-7 |
| navigation/listing | `^(navigation\|listing)-` | A2-1, A4-4 |
| selection | `^selection-` | A2-1 |
| Properties | `^properties-` | A2-2, A4-1 |
| input/create/rename | `^(input\|create\|creation\|rename)-` | A0-2, A2-1, A2-2 |
| copy/move/fileops | `^(copy\|move\|fileops)-` | A0-2, A0-3 |
| delete | `^delete-` | A0-2, A0-3, A4-3 |
| access/guards/links | `^(access\|guards\|links)-` | A0-2, A0-3, A4-1 |
| names/limits/contents/trees | `^(names\|limits\|contents\|trees)-` | A0-2, A2-1, A2-3 |

## Recorded evidence and boundaries

- Foundation/input/navigation/listing/selection/create/edit/Properties/history:
  [foundation fixes](runs/2026-10-05-native-foundation-fixes.md),
  [navigation](runs/2026-10-05-native-navigation.md),
  [listing/search](runs/2026-10-05-native-listing.md),
  [selection](runs/2026-10-05-native-selection.md),
  [creation](runs/2026-10-05-native-creation.md),
  [editing/history/Properties](runs/2026-10-05-native-editing-history-properties.md).
- Transfer/conflict/unsafe-target/batch families:
  [NT2 records](runs/2026-10-06-native-transfers.md); exact provider/direction,
  part status, app-source identity and remaining scope stay in those records.
- Progress/cancel/overwrite/access/I/O failure/concurrent writers/interruption:
  [NT3 records](runs/2026-10-06-native-failure-safety.md). Injected fixture failures
  do not become real full-disk, outage, power-loss or disconnected-media evidence.
- Name/encoding/size/tree/link families:
  [NT4 records](runs/2026-10-06-native-name-data-edges.md); representative bounded
  device trees do not become broad device stress acceptance.
- USB/SFTP/MTP/OneDrive families:
  [NT5 provider records](runs/2026-10-06-native-provider-behavior.md). Lifecycle
  transitions remain separately approved and NOT_RUN where only a contract exists.
- Private desktop drag/clipboard/trash/keyboard/appearance/watchers/archive/handler:
  [NT6 records](runs/2026-10-07-native-desktop-interaction.md). Private X11,
  Nautilus copy and generated dummy associations do not accept personal Wayland
  services, real defaults/apps, format/eject or every display/accessibility mode.
- Fresh/reused smoke and performance samples:
  [NT7 records](runs/2026-10-07-native-repeatability.md). Host budgets remain
  NOT_AGREED; dedicated external CI execution remains NOT_RUN.

Actual PASS needs the declared case/part/provider scope, independent effects,
owned executable/profile/process identity, resource release where required, and
confirmed teardown/private retention. Preserve original FAIL/BLOCKED reports;
a later fresh PASS does not rewrite them or turn NOT_RUN into PASS. Raw reports,
screenshots, account configurations and fixture paths remain in private ignored
owned runs. Public records keep UUIDs, hashes, counts, scopes and limitations.

Only concrete reproduced product defects enter the active engineering backlog.
NT7 reproduced harness/input assumptions and a timing-boundary error, no new
Browsey defect. Existing parent rows and release/ordinary-use signoff stay open
according to their original scope. The native-suite development/execution track
is [archived](../../todo-archive/TODO_NATIVE_TEST_SUITE.md) once its declared
NT0–NT7 deliverables and bounded verification are complete.

# Core Operations Release Checklist

Created: 2026-03-02
Derived from: `docs/operations/core-operations/matrix.md`
Purpose: Run trust-critical validation for a release candidate without
redefining behavior in multiple places.

## Run Metadata

- [ ] Date:
- [ ] Candidate version/build:
- [ ] Commit SHA:
- [ ] Tester:
- [ ] OS + distro/version:
- [ ] `rclone version` (if cloud scenarios executed):
- [ ] Notes link (logs/screenshots/issues):

## Preconditions

- [ ] Build under test launches successfully.
- [ ] Linux-first core scenarios are included in every release candidate run.
- [ ] Windows scenarios are included when touched areas claim Windows behavior.
- [ ] Cloud scenarios run only when `rclone` is installed/configured and a
      disposable test remote is available.
- [ ] Test data set includes:
  - at least one file conflict pair
  - at least one directory conflict tree
  - at least one archive with multiple entries

## Execution Rules

1. For each scenario row, run the action and mark `PASS` or `FAIL`.
2. Expected behavior is defined only in `docs/operations/core-operations/matrix.md` for
   the same scenario ID.
3. A `FAIL` on any release-blocking scenario must be linked to an issue before
   signoff.
   release-blocking definition: `docs/operations/core-operations/release-blocking-policy.md`
4. If a scenario is not applicable to the touched scope, mark `N/A` and justify
   in notes.

## Matrix-Derived Validation Rows

### Local clipboard-backed copy/move (`LCM`)

| Done | Scenario ID | Platform | Action to execute | Result (`PASS`/`FAIL`/`N/A`) | Notes |
|---|---|---|---|---|---|
| [ ] | `CO-LCM-001` | Linux + Windows* | Copy one local file into a different folder. |  |  |
| [ ] | `CO-LCM-002` | Linux + Windows* | Copy one local folder tree into a different folder. |  |  |
| [ ] | `CO-LCM-003` | Linux + Windows* | Move one local file into a different folder. |  |  |
| [ ] | `CO-LCM-004` | Linux + Windows* | Move one local folder tree into a different folder. |  |  |

### Local rename (`LRN`)

| Done | Scenario ID | Platform | Action to execute | Result (`PASS`/`FAIL`/`N/A`) | Notes |
|---|---|---|---|---|---|
| [ ] | `CO-LRN-001` | Linux + Windows* | Rename a file to a unique target name. |  |  |
| [ ] | `CO-LRN-002` | Linux + Windows* | Rename a folder to a unique target name. |  |  |
| [ ] | `CO-LRN-003` | Linux + Windows* | Trigger rename conflict and validate no-overwrite/explicit conflict behavior. |  |  |

### Local trash/delete/restore/purge (`LTD`)

| Done | Scenario ID | Platform | Action to execute | Result (`PASS`/`FAIL`/`N/A`) | Notes |
|---|---|---|---|---|---|
| [ ] | `CO-LTD-001` | Linux + Windows* | Send one file to trash. |  |  |
| [ ] | `CO-LTD-002` | Linux + Windows* | Send one non-empty folder to trash. |  |  |
| [ ] | `CO-LTD-003` | Linux + Windows* | Restore one item from trash back to origin/fallback. |  |  |
| [ ] | `CO-LTD-004` | Linux + Windows* | Purge one item permanently from trash view. |  |  |
| [ ] | `CO-LTD-005` | Linux + Windows* | Permanently delete one file/folder (bypass trash). |  |  |

### Mixed local<->cloud transfer + conflict preview (`MTC`)

| Done | Scenario ID | Platform | Action to execute | Result (`PASS`/`FAIL`/`N/A`) | Notes |
|---|---|---|---|---|---|
| [ ] | `CO-MTC-001` | Linux cloud path | Copy local file to cloud folder. |  |  |
| [ ] | `CO-MTC-002` | Linux cloud path | Copy cloud file to local folder. |  |  |
| [ ] | `CO-MTC-003` | Linux cloud path | Move local file to cloud folder. |  |  |
| [ ] | `CO-MTC-004` | Linux cloud path | Move cloud file to local folder. |  |  |
| [ ] | `CO-MTC-005` | Linux cloud path | Copy or move a directory tree across local/cloud boundary. |  |  |
| [ ] | `CO-MTC-006` | Linux cloud path | Run mixed conflict preview and verify chosen policy in execute phase. |  |  |

### Archive extraction (`EXT`)

| Done | Scenario ID | Platform | Action to execute | Result (`PASS`/`FAIL`/`N/A`) | Notes |
|---|---|---|---|---|---|
| [ ] | `CO-EXT-001` | Linux + Windows* | Extract archive to a new destination. |  |  |
| [ ] | `CO-EXT-002` | Linux + Windows* | Extract archive into a conflicting destination. |  |  |
| [ ] | `CO-EXT-003` | Linux + Windows* | Cancel extraction while extraction is in progress. |  |  |
| [ ] | `CO-EXT-004` | Linux + Windows* | Force permission-denied during extraction write path. |  |  |
| [ ] | `CO-EXT-005` | Linux + Windows* | Run batch extraction with mixed success/failure outcomes. |  |  |

#### Extract Non-Transactional Notes

- Extraction is not globally transactional across an entire archive or archive
  batch.
- On cancellation/failure, completed outputs from earlier completed entries (or
  earlier archives in a batch) may remain.
- Partial output created by the currently failing entry is best-effort cleaned
  up by backend-created path tracking, but callers must validate final
  filesystem state via post-operation refresh/checks.
- Release validation for `CO-EXT-003` to `CO-EXT-005` must confirm user-facing
  summary and observed filesystem state agree on partial vs full success.

## Environment Notes

- Linux-first required checks:
  - `LCM`, `LRN`, `LTD`, and `EXT` families must run for every release
    candidate.
  - `MTC` family must run when cloud transfer/refresh/conflict code is touched.
- Windows checks:
  - Required only when touched modules include Windows-claimed behavior
    (especially `src/commands/fs/windows.rs`, delete/trash/permanent-delete
    flows, or Windows packaging/runtime changes).
  - If skipped, log justification in run notes.
- Cloud preconditions:
  - `rclone` installed and in `PATH`.
  - At least one configured remote (OneDrive primary v1 target).
  - Disposable test directory and representative test files.

## Provider-Specific Appendices

Provider-specific real-account validation remains separate from this
matrix-derived core checklist to avoid duplicating product semantics.

- OneDrive real-account appendix:
  `docs/cloud/checklists/onedrive-rclone-v1-manual-checklist.md`

Use the appendix for provider anomalies, quota/rate-limit behavior, and
ecosystem-specific quirks after core `MTC` scenarios pass.

---

\* `Linux + Windows` means Windows is required when the touched scope claims
Windows support or changes Windows-specific behavior.

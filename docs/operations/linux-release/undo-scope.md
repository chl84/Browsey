# Linux 1.0 Undo/Redo Scope

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Status: Active source of truth for the Linux 1.0 undo/redo claim.

## Purpose

Define the actual undo/redo scope that Browsey supports for the Linux 1.0
production claim, so release validation and user-facing docs do not imply a
broader guarantee than the code currently provides.

This document is intentionally narrower than a full feature inventory. It only
states which actions are part of the Linux 1.0 undo/redo claim, which
boundaries apply, and which areas are explicitly outside that claim.

## Core Guarantees

For the Linux 1.0 production claim, Browsey undo/redo is defined as:

- local-operation only
- in-memory only for the lifetime of the running app
- capped to the most recent 50 recorded actions
- cleared forward-redo history when a new action is recorded

This means Browsey may only claim Linux 1.0 undo/redo support for actions that
are explicitly recorded into the shared `UndoState` and replayed through the
central undo engine.

## Supported Undo/Redo Scope for Linux 1.0

The following local filesystem actions are inside the Linux 1.0 undo/redo
claim:

- copy and cut/paste operations that complete through the clipboard file-op
  pipeline
- rename and batch rename for local filesystem paths
- create file
- create folder
- permanent delete flows that move the original path into the undo backup area
- move to trash flows that record either the final trash move or the backup
  fallback action
- archive compression when the operation creates a new archive successfully
- archive extraction when the operation creates output successfully

These are the concrete action types currently recorded into the shared undo
history on Linux:

- `Action::Copy`
- `Action::Move`
- `Action::Rename`
- `Action::Create`
- `Action::Delete`
- `Action::CreateFolder`
- `Action::Batch(...)`

## Boundaries That Must Be Documented Clearly

The Linux 1.0 undo/redo claim is subject to these hard boundaries:

- undo/redo history is not persisted across app restart
- startup cleanup may remove stale undo backups from previous runs
- only actions that successfully completed and were recorded are undoable
- multi-item operations may be recorded as one `Batch(...)` history item rather
  than many separate undo steps
- a newly recorded action clears redo history
- history depth is capped at 50 recorded items

These boundaries are part of the supported behavior, not incidental
implementation details.

## Outside the Linux 1.0 Undo/Redo Claim

The following areas are explicitly outside the Linux 1.0 undo/redo claim unless
the code and this document are both updated later:

- all cloud (`rclone`) operations
- search
- open/open-with launches
- settings changes
- mount/discovery/network connection flows
- permissions editing and ownership changes
- any operation that does not record an action into the shared `UndoState`

Cloud is especially important here: Browsey currently advertises
`can_undo: false` for cloud capabilities, and Linux 1.0 must continue to treat
cloud undo/redo as unsupported.

## Release Validation Use

This document is the Linux 1.0 source of truth for undo/redo scope. Release
validation, bugbash work, and user-facing Linux docs should use it to answer:

- which workflows must keep working with undo/redo on Linux
- which workflows must not be described as undoable
- which regressions count as Linux trust regressions versus expected scope

If Browsey expands undo/redo later, the release checklist and this document
must be updated together.

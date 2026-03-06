# Linux Runtime Surface Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 4 Linux-specific runtime behavior outside the already-audited
optional-dependency slice.

## Purpose

Assess the current Linux-specific evidence for:

- terminal launch
- permissions and ownership editing
- trash behavior

This audit is intentionally evidence-driven. It does not claim these areas are
finished for Linux 1.0; it records what is already underpinned in code/tests
and what still needs explicit Linux validation.

## Evidence Reviewed

- `src/commands/console.rs`
- `src/commands/permissions/tests.rs`
- `src/commands/permissions/set_permissions.rs`
- `src/commands/permissions/ownership/unix.rs`
- `src/commands/fs/trash/tests.rs`
- `docs/audits/core-operations/local-gap-audit.md`
- `docs/audits/linux-release/core-workflow-gap-audit.md`
- `docs/operations/linux-release/bugbash-checklist.md`

## Status Summary

| Area | Current Linux 1.0 status | Basis |
|---|---|---|
| Terminal launch | Partial | Linux terminal launch uses a strict allowlist and typed failure paths, but there is no explicit Linux 1.0 validation yet across real desktop environments. |
| Permissions / ownership editing | Partial | Backend permission/ownership logic has meaningful validation and rollback-oriented tests, but Linux 1.0 still lacks end-to-end UI/runtime validation on representative real paths. |
| Trash behavior | Partial | Trash move/restore/purge logic has strong backend tests and rollback coverage, but the local gap audit still records remaining hostile-condition gaps and Linux manual validation is not yet closed. |

## Terminal Launch

Current evidence is solid at the implementation level:

- Linux terminal launch uses a strict allowlist of supported terminal commands.
- The command path is not environment-controlled or free-form.
- Failures map to typed console error codes, including a specific
  `terminal_unavailable` path when no supported terminal is found.

What is still missing for Linux 1.0:

- manual validation on the supported Linux target surface, especially GNOME
  Wayland
- confirmation that the allowlisted terminals behave acceptably in installed
  builds, not just in dev/runtime assumptions

## Permissions and Ownership Editing

Current backend evidence is meaningful:

- permission toggles cover read-only, executable, and owner/group/other access
  updates
- relative paths and symlink-containing paths are rejected
- later-target failure rolls back earlier permission changes
- permission and ownership changes are explicitly kept out of undo history
- ownership validation rejects missing principals and supports no-op current-id
  application

What is still missing for Linux 1.0:

- end-to-end UI validation that Properties open/edit/apply flows behave correctly
  on real Linux paths
- runtime validation of privilege-escalation paths where ownership change needs
  pkexec/elevated handling
- manual validation that the resulting refresh/error states feel sane in the
  actual Linux UI

## Trash Behavior

Current backend evidence is also meaningful:

- single-item trash move is validated through backend abstraction tests
- multi-item trash move rollback on later failure is covered
- stale staged trash recovery is covered
- restore and purge success/failure event behavior is covered
- original-path icon mapping for trash entries is covered

What remains open for Linux 1.0:

- the local gap audit still records restore-conflict and additional hostile
  backend-condition coverage gaps
- Linux runtime behavior still needs explicit validation in the actual app on
  representative systems, not only backend fake-ops tests

## Conclusion

This audit does not justify checking off
`Test terminal launch, permissions editing, and trash behavior on common Linux environments`
yet.

It does justify treating the area as:

- backend-rich and worth validating manually next
- no longer vague; the remaining Linux 1.0 work is primarily end-to-end/runtime
  confirmation rather than first-principles design

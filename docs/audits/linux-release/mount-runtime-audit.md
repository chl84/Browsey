# Linux Mount Runtime Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 4 Linux-specific validation for mounts, eject, removable media, and
GVFS-backed endpoints.

## Purpose

Record the current Linux-specific evidence for:

- local/removable mount listing
- eject/unmount behavior
- GVFS-backed endpoints, including network mounts and user-space devices

This audit is intentionally evidence-driven. It exists to make the remaining
Linux 1.0 work concrete instead of leaving `mounts/eject/GVFS` as a vague
integration bucket.

## Evidence Reviewed

- `src/commands/network/mounts.rs`
- `src/commands/network/gio_mounts.rs`
- `src/commands/network/uri.rs`
- `src/commands/listing/watch.rs`
- `src/entry/mod.rs`
- `docs/operations/linux-release/bugbash-checklist.md`
- `docs/operations/core-operations/release-checklist.md`

## Status Summary

| Area | Current Linux 1.0 status | Basis |
|---|---|---|
| Local/removable mount listing | Partial | Linux mount listing is implemented from `/proc/self/mounts` with removable heuristics and GVFS root filtering, but has no dedicated backend tests in `mounts.rs` and no Linux 1.0 manual validation evidence yet. |
| Eject/unmount behavior | Partial | The Linux eject flow has a clear strategy (`gio mount -u` -> `umount` -> `udisksctl` -> optional lazy unmount) and typed `eject_failed` handling, but there is no isolated automated coverage or installed-build bugbash evidence yet. |
| GVFS/network-backed endpoints | Partial | Browsey already treats GVFS paths specially in listing/watch/open/clipboard-adjacent code, but GNOME Wayland/GVFS scenarios still lack explicit Linux 1.0 validation against real mounts. |

## Mount Listing

Current evidence:

- Linux mount enumeration reads `/proc/self/mounts`.
- Pseudo/system mounts are filtered out aggressively.
- GVFS-backed mounts are kept, but the generic GVFS root is hidden from user
  partitions.
- Removable hints are derived from mount roots, filesystem types, and block
  device naming heuristics.

What is still missing:

- dedicated tests for Linux mount filtering and removable heuristics
- validation against real removable media and user-visible mount roots on the
  supported Linux target surface

## Eject and Unmount

Current evidence:

- The eject path is conservative and ordered:
  - try `gio mount -u`
  - fall back to `umount`
  - then `udisksctl unmount -b`
  - optionally use lazy unmount for busy volumes
- Watchers are dropped before unmount attempts.
- Discovery cache invalidation is explicit on successful disconnect paths.
- Busy-volume detection produces a specific user-facing guidance message instead
  of a raw shell dump.

What is still missing:

- automated coverage for Linux eject fallbacks and busy-volume branches
- manual validation on removable media and GVFS-backed user mounts
- confirmation that device power-off behavior is acceptable on supported Linux
  targets

## GVFS and Network-backed Mounts

Current evidence:

- GVFS mount roots are surfaced from `gio_mounts`.
- URI-to-mounted-path resolution already accounts for GVFS mount parameters.
- Watch roots include the GVFS runtime directory when present.
- Entry classification already treats GVFS-backed mounts as network-like in the
  UI where appropriate.

What is still missing:

- end-to-end validation on GNOME Wayland with representative SMB/NFS/GVFS
  targets
- confirmation that connect/mount/done states feel sane in the installed app
  rather than only in code-level assumptions

## Conclusion

This audit does not justify checking off either of these TODO items yet:

- `Test mounts, eject, and removable media against real Linux setups`
- `Test SMB/NFS/GVFS scenarios on GNOME Wayland`

It does justify the next concrete Linux 1.0 work:

- add dedicated tests around Linux mount filtering/eject fallbacks where
  practical
- extend the Linux bugbash checklist with mount/eject/GVFS rows
- run manual validation on supported Linux targets instead of treating the area
  as implicitly covered by generic file-browsing tests

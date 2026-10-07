# Google Drive object identity

Implemented in the 2026-10-08 working tree. The installed 1.0.4 reproduction is
recorded in the [provider-difference run](../operations/linux-release/runs/2026-10-07-google-drive-provider-differences.md).
The [candidate acceptance run](../operations/linux-release/runs/2026-10-08-google-drive-object-identity.md)
records native identity checks, regression results and performance measurements.

## Implementation sequence and scope

1. Retain the IDs already returned by rclone's directory listing. Give each
   Drive object a distinct backend-issued path so selection, refresh,
   clipboard, search and durable working copies retain the selected identity.
2. Resolve directory access through an operation-local `root_folder_id`.
   Transfer individual files through rclone `copyid`/`moveid`; retain rclone's
   native-document exports and shortcut behavior.
3. Address metadata, trash, permanent deletion and same-remote directory moves
   by Drive API object ID. Keep rclone responsible for OAuth refresh. Refuse
   ambiguous name-only references and unsafe bulk operations before writes.
4. Decode names for the path field, breadcrumbs and Properties while keeping
   the complete object reference in actions and history. Keep the existing
   frontend reconciliation and selection algorithms.
5. Verify distinct listing rows, exact selected-file contents, independent
   directory navigation and selected-object mutation. Run shared provider,
   transfer and frontend regressions and an optimized local listing workload.

This is primarily a backend change. The frontend needs a small shared path
helper because an object's display name is no longer its complete identity.

## References and operations

An internal address has this form:

```text
rclone://Remote//gdrive/parentID~Folder/fileID~same.txt
```

The reserved double slash distinguishes this from an ordinary cloud path.
Names are URL encoded; empty ID segments represent new destination names or
unresolved ancestors. The backend validates both the ID alphabet and decoded
names. Shortcut references retain the target and shortcut IDs separately:
directory access/downloads use the target, while trash/delete/move address the
shortcut object. IDs identify objects; they are not authentication credentials.

The listing operation already supplies all IDs. It makes no per-entry metadata
requests. Legacy paths and typed paths resolve one directory at a time and
refuse a name matching more than one object. Already selected references do
not require name-based source traversal.

The Drive API HTTP client is initialized lazily and pools connections. OAuth
tokens are cached until near expiry, cleared with provider configuration
changes, and held in zeroizing storage. Token refresh runs a read-only rclone
command using the configured remote's existing authentication. Requests have
bounded timeouts; uncertain mutations are not automatically replayed.

## Performance boundaries

Local file listing/copy/move, the ordinary rclone listing item layout, the
frontend entry model and directory reconciliation algorithms are unchanged.
Other providers do not parse or allocate Drive IDs, run the duplicate-ID check,
initialize the HTTP client, or use Drive API requests. Ordinary cloud paths
allocate no ID vector. The ID-scoped rclone connection is per operation and
does not modify the remote configuration.

Only Google Drive directory transfers add a recursive `lsjson --fast-list`
metadata pass. It checks the source, and an existing destination when relevant,
for duplicate descendant paths before a bulk transfer. This cost belongs to
the Google-specific safety check; it is not added to ordinary file transfers.

## Explicit supported boundaries

- A directory tree containing identical descendant names is refused before
  bulk copy/move. Select individual objects and use unique destination names.
  The existing rclone bulk transfer engine cannot preserve both objects at one
  ordinary destination path.
- Overwriting one of several existing same-name files is refused, even when
  the caller supplies a selected destination ID. rclone `copyid` chooses an
  existing destination by name. Unique destinations retain normal overwrite
  behavior through the standard rclone engine after source and target names
  are verified against their IDs. Duplicate sources require a unique
  destination or Auto-rename: `copyid` passes no existing destination object
  to rclone and cannot safely implement overwrite by itself. A disappeared or
  renamed selected destination is refused rather than recreated by its old name.
- ID mutations/metadata through the Drive API require an OAuth rclone remote.
  Service-account/ADC remotes retain listing, selected-ID metadata through
  parent listings and rclone file downloads, but direct ID trash/delete/folder
  moves return an explicit unsupported error. They are not silently retried
  by name. Real-provider validation uses the existing OAuth desktop remote.
- Directory rename through the API supports the default rclone Drive encoding.
  Custom encoding is refused for a changed directory name. File rename uses
  rclone `moveid`, including its native-document extension rules.
- Provider-side changes after a preflight are not transactional. Existing
  concurrent-writer limitations still apply; this change does not introduce
  remote compare-and-swap, recovery receipts or cloud undo.

Provider mechanisms: [rclone Drive backend commands](https://rclone.org/drive/#backend-commands),
[RC backend/command](https://rclone.org/rc/#backend-command),
[Drive files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update),
[Drive files.delete](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/delete)
and [rclone name encoding](https://rclone.org/overview/#encoding).

# Editing cloud files

When you open a supported cloud file normally or with Open With, Browsey creates
an independent private working copy. Save in your editor as usual. Browsey watches
that copy's directory and waits 1.5 seconds after changes before uploading a stable
snapshot to the original. Keep Browsey running. **Cloud saves · Saved** means the
provider confirmed the save; completed byte progress alone does not.
The Saved badge disappears after four seconds. Close a need-attention notice with
its × button; new or changed problems show a new notice. Pending saves remain
visible, and working-copy status and recovery details stay in Settings.

The original object ID is retained for Google Drive and OneDrive. Identical Google
Drive names remain separate. Each working copy has its own version baseline, so
editing the same cloud object through two independently opened copies can produce
a conflict. Versions are checked before uploading, and the upload itself includes
`If-Match`; a concurrent cloud change is rejected rather than silently overwritten.

Open **Cloud saves**, or **Settings > Cloud > Working copies**, to inspect status,
open your local copy, pause/resume automatic saving, retry **Save to original**, or
choose **Save as new file**. Pausing takes effect after any current upload
finishes. Pause automatic saving before editing if you want to upload only a new
cloud file. Saving under a different local filename in your
editor does not upload that new file automatically. A conflict preserves the local
file and upload snapshot.
Compare versions, reopen the current original, or save under a new name. Retrying
does not force an overwrite or discard the original version check.

Connection failures retain a durable snapshot and retry with bounded backoff.
If a response is lost after the provider committed the upload, Browsey verifies the
snapshot's bytes against a stable cloud revision before treating it as saved.
New edits made during upload remain pending for the next save. Copies and pending
journals survive restart and preview-cache clearing. Earlier working copies stay
manual: explicitly saving to their original first verifies the original bytes.
Archive staging and external export copies remain manual.

## Local storage and manual cleanup

**Settings > Cloud > Working copies** shows the total size of stored file contents
and the size of each copy, including its metadata. Storage inspection runs only
when you open or refresh this dialog. Staging and unreadable recovery entries are
included in the total and retained; linked targets are not followed.

Use a copy's **Actions…** menu to open it, show its folder, save, pause/resume
automatic saving or choose **Remove local copy…**. Unavailable actions stay visible
but disabled. The shared ComboBox keeps the action placeholder after a choice and
supports keyboard navigation; Escape closes the list before the dialog.

Use **Storage actions… > Clean up saved copies…**
for the currently listed eligible copies marked Saved. Close the selected files in
your editor before confirming **Files closed — move to trash**. Browsey cannot
reliably detect whether an external editor still has a copy open.

Cleanup checks the current file bytes again and skips active saves, pending
journals, conflicts, errors, modified copies, links and additional editor or
recovery files. Each skipped copy has an explanation. Only the explicitly selected
local session folders move to the system trash; cleanup requires no cloud
connection and never changes cloud originals. There is no scheduled eviction or
permanent-delete fallback if the trash operation fails.

Copies in the trash still use disk space until you empty the trash. Each trashed
folder contains its file and manifest. Cleanup first detaches the session into a
private cleanup folder; to recover it as a Browsey working copy, put the complete
ID-named folder back into the working-copy storage root and restart Browsey.
Interrupted cleanup folders and failed rollback output stay available for manual
recovery through **Storage actions… > Show storage folder**.

## Supported routes and limits

- Google Drive: ordinary binary files on OAuth rclone remotes, addressed by ID.
  Native Google Docs/Sheets/Slides, exported representations and shortcuts
  require manual handling. Opening their export does not replace the
  native document. Unsupported authentication retains the local copy.
- OneDrive: standard global Microsoft Graph OAuth remotes, addressed by drive/item
  ID. Automatic saving uses the conditional simple upload route for files up to
  250 MB. Larger files retain their edits and require save-as-new/manual handling.
- Nextcloud: standard HTTPS WebDAV rclone configuration with a strong ETag and
  supported default encoding, basic credentials or a bearer token. Custom headers,
  custom encodings, weak validators and unverified configurations stay manual.

The watcher is lazy and nonrecursive, scoped to editor-copy directories. It ignores
read/access notifications and performs no periodic cloud polling or idle hashing.
Hashing, snapshotting and network work run outside the main thread and registry
locks. Provider concurrency limits still apply. Browsey cannot track editor writes
while it is closed; automatic copies are checked when Browsey starts again. Cloud
listing refresh remains explicit, and this feature does not add cloud undo.

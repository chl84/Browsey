# Using Browsey

[Back to README](../README.md) · [Installation](installation.md) ·
[Documentation site](https://chl84.github.io/Browsey/)

## Navigation and shortcuts

Properties disables permission and ownership edits on read-only mounts, and
shows a short explanation when mount options or the server control access.
On Linux, NTFS3 supports normal edits unless its access rules are disabled;
NTFS-3G requires confirmed `permissions`/`acl` support without `inherit`.
Unidentified `fuseblk` mounts are conservatively non-editable. NFS, SSHFS and
SMB mounts with reported Unix/POSIX or ACL support remain editable, subject to
server authorization. Browsey verifies the returned permission/ownership state;
it does not remount drives or change server policy.

On Linux, the Partitions sidebar shows total block-volume capacity in decimal
GB beside each known local volume. This is not free space. Mounts sharing the
root volume (such as Btrfs subvolumes) show capacity only at `/`; separate
partitions and USB volumes retain their own capacity labels. Unknown capacities
are hidden, including phone and network endpoints.

Right-click any partition and choose **Properties** (or use the context-menu key
or Shift+F10 while its row is focused). On Linux, **Basic** shows total, used, and
free filesystem space for mounted local volumes using the current theme. Free
space excludes filesystem reservations; a tooltip explains any reserved space.
These statistics are read when the dialog opens, without counting files.
Unmounted devices are not mounted automatically, and phone/network storage is
not probed for local usage. Ownership and permission changes still affect only
the mount root, not its contents.

Defaults are remappable in Settings:

- `Ctrl+F` search; `Ctrl+G` switch between list and grid.
- `Ctrl+A` select all; `Ctrl+C/X/V` copy, cut, and paste.
- `Ctrl+R` rename; `Ctrl+P` properties.
- `Delete` trash; `Shift+Delete` permanently delete.
- `Ctrl+H` hidden files; `Ctrl+B` bookmarks; `Ctrl+T` terminal.
- `Esc` exits search/filter contexts.

Search starts in the current folder and includes its subfolders on local,
mounted network/mobile and cloud storage. Enter submits the query. Changing
the query clears previous results until it is submitted again. Cloud search
reads names and metadata through rclone without downloading file contents;
typing a folder filter only narrows the current folder's visible entries.

In Settings and Properties dropdowns, arrow keys keep the highlighted option
visible. Escape closes an open dropdown and returns focus to its button;
press Escape again to close the dialog. The same behavior applies to searchable
user/group dropdowns in Properties.

Ctrl + mouse wheel zooms between list and five grid sizes (64, 96, 128, 160,
and 192 CSS pixels). Zoom is window-local and resets on restart. Grid gaps
are 8 px in Cozy and 6 px in Compact.

Column filters remain active when switching to grid view. Use **Reset** beside
**Column filters active** to clear all column filters without changing text
search, sorting, or hidden-file visibility.

Launch with `browsey /path/to/folder` or `browsey 'file:///path/to/folder'` to
open a particular directory. Relative paths resolve from the launch working
directory. With no argument, Settings > Start folder applies. Only one folder
per launch is supported; use `browsey -- -folder` for a relative name starting
with a dash. Invalid arguments report an error rather than silently opening Home.

On Hyprland, the titlebar retains Menu and Close but hides Minimize and
Maximize; use the compositor's controls instead. Other desktops retain the
normal buttons. Browsey does not change compositor settings.

## Drag and drop

- Drop onto a folder, breadcrumb, bookmark, mounted drive, or empty space in
  a normal directory view. Files, unmounted drives, search/virtual backgrounds,
  and views with an open dialog are not destinations.
- Hold Ctrl/Meta before starting a local drag to lock copy, or Shift to lock
  move. Ctrl/Meta wins if both are held. Otherwise internal drops use live
  modifiers and filesystem-aware defaults: move on the same filesystem,
  copy across filesystems. Cloud transfers default to copy.
- Incoming drops from another app copy to the folder under the pointer.
- Drop onto **Wastebasket** to move items to their storage's trash. Local and
  removable volumes use system trash; Google Drive and OneDrive use provider
  trash (restore there). Network and device mounts require reported trash
  support. Unsupported drops never fall back to permanent deletion. Existing
  undo/redo behavior is retained; cloud and remote trash do not gain local undo.
  Copy-only drags started with Ctrl/Meta are rejected by Wastebasket; start a
  normal drag to trash instead. Wastebasket never opens automatically on hover.
- Hover over a destination for 850 ms to open it; drag near list/grid/sidebar
  edges to scroll. Escape cancels internal dragging.
- Drag local files to another app without Alt. Without a start modifier,
  the receiving app chooses copy/move. Browsey does not delete sources merely
  because a drag completes. For cloud items, use **Prepare external copy…**,
  then drag the prepared button; exports are copy-only. A single selection
  cannot mix local and cloud sources.

See [native drag tests and acceptance](testing-native-drag.md) for coverage.

## Open With and archives

On Linux, choose an app in **Open With**, check **Set as default**, then press
**Open** to set the desktop handler for all files of that MIME type. Leaving
the checkbox unchecked is a one-time opening. Directories and unknown types
cannot change their defaults through this action.

Archive creation is ZIP-only, with optional AES-256 passwords. Filenames and
directory structure remain visible; readers must support WinZip AES.
Extraction supports password-protected ZIP (AES/ZipCrypto), 7z, and RAR,
including encrypted 7z/RAR headers. Batch prompts are per archive; wrong
passwords can be retried or cancelled. Passwords are not saved in settings,
history, or logs.

Extraction is not globally transactional. Completed outputs may remain after
a batch error or cancellation. Upper limits are 100 GB of extracted contents
and 2,000,000 entries; disk-space checks may stop extraction earlier.
Private output permissions, unsupported
special files, and password-memory boundaries are described in
[archive tests and limitations](testing-archives.md).

## Cloud storage

Install rclone and configure a remote with `rclone config`, then enable
Settings > Cloud. Supported providers are OneDrive, Google Drive, and
Nextcloud (recognized Nextcloud WebDAV remotes). There is no in-app account
login. Browsey detects rclone in PATH or uses Settings > Cloud > Rclone path.
**Test connection** checks the remote's RC and CLI read paths.

Remotes appear in Network; direct navigation accepts
`rclone://REMOTE/path`. Cloud listings do not have filesystem-watch live
refresh; explicit/manual refresh is needed in some workflows.

- **New File**, **Open With**, and preflighted advanced rename are available.
  Batch rename stops at the first execution failure and reports partial
  completion; it is not transactional.
- Opening a cloud file creates a private persistent working copy, preserving
  its filename. Manage retained data in Settings > Cloud > Working copies.
  Supported newly opened files automatically save back to the same cloud object
  after a local editor save. Keep Browsey running and wait for **Cloud saves · Saved**.
  Conflicts stop saving and preserve local edits; connection failures keep a
  durable snapshot for retry. **Save as new file** remains available. Older copies
  stay manual. Native Google documents/exports and unsupported configurations
  require manual upload. See [cloud editing](cloud/cloud-editing.md) for limits
  and recovery. Copies survive restart and preview-cache clearing.
- **Compress** and **Extract** use protected local staging and the existing
  archive engine/password flow. ZIP creation uploads a new archive; extraction
  uploads a uniquely named folder. Originals/staging remain on failure or
  cancellation. Symlink/special-file uploads are refused. Archive/export
  selections must come from one cloud folder.
- **Move to cloud trash** is available for OneDrive and Google Drive. Restore
  through the provider website; local Wastebasket does not show cloud trash.
  Nextcloud trash is not exposed. OneDrive Personal hard-delete is unsupported,
  and provider/server retention policies can retain explicitly deleted data.
- **Prepare external copy…** downloads items before copy-only export to another
  app. Cloud originals are never removed by external drag.
- **Cloud thumbs** is opt-in; currently limited to Grid view for image/pdf/svg,
  with provider and file-size guardrails.

There is no cloud undo/redo, duplicate scan, automatic sync, or atomic
conditional overwrite. Stop other writers during operations. Protected copies
and staging have no automatic retention or disk quota; remove them only after
closing editors and finishing operations. The old `cloud-open` cache is left
untouched because it may contain earlier edits.

Real-provider expansion acceptance is partial for OneDrive and pending for
Google Drive/Nextcloud, not implied by mocked tests. See the
[provider checklists](cloud/checklists/) and version-specific release notes.

For repeated cloud listing stalls, set log level to Debug and inspect
`browsey/logs/browsey.log` under the app data directory. Launching with
`BROWSEY_RCLONE_RC=0` can isolate RC-daemon vs CLI behavior. Do not automatically
repeat an ambiguous write; refresh and inspect its destination first.

## USB drives and phones

Right-click a removable USB drive for mounting, Properties, or Format.
Formatting supports exFAT, FAT32, ext4, btrfs, and NTFS when matching system tools
are installed. Formatting erases the selected device: verify its identity
and keep backups. New ext4/btrfs roots are made writable by the formatting
user; existing volumes are not automatically re-owned.

Unavailable filesystems remain visible but disabled, with the missing utility
shown below the selector. NTFS requires `mkntfs`. Reopen the dialog after
installing tools. Volume names accept ASCII letters, numbers, spaces, hyphens,
and underscores: up to 128 characters for NTFS, or 11 for the other formats.
Switching formats preserves the name but requires correcting it if it is too long.

Progress uses real UDisks percentages when available, otherwise indeterminate
progress. If completion is uncertain, inspect the device before retrying;
Browsey never automatically repeats an erase.

MTP phones are discovered through GIO and mounted on demand. Keep the phone
unlocked with USB file-transfer mode enabled. Temporary I/O errors may require
a connection check and retry. Phone Properties are informational; formatting
and POSIX permission editing are not offered. GVFS/MTP fallback moves without
verifiable output ownership may copy but refuse source deletion; inspect both
sides rather than automatically repeating the move.

Automatic refresh in GVFS/MTP folders keeps existing entries in place while
metadata and thumbnails arrive. New files are appended and deleted files are
removed; cached details remain visible while fresh metadata is pending. Use F5
or change the sort setting to apply the current sorting again. Sorting uses the
metadata currently available; a newly opened phone folder may still be resolving
dates and sizes. Background refresh does not interrupt navigation or search.

## File safety and recovery

Keep backups, verify paths before destructive actions, and stop other writers
during copy/move/undo. Concurrent-edit detection is not a transaction guarantee;
see the [supported boundary](audits/daily-driver/concurrent-writer-boundary.md).

Undo holds up to 50 actions for this session only, not a storage quota or
persistent history. Original overwrite destinations are protected with recovery
markers before being moved to backups. Failed/cancelled copies can leave
uncertain output for inspection instead of deleting another program's edits.
Blocked restoration retains backups and reports their paths. This is manual
recovery, not automatic retry or power-loss durability.

Local manual copies compare output against the written stream before accepting
completion or deleting fallback move sources. The extra target-read pass is
not a lock against later edits; GIO/cloud copy paths are different.

Settings > Stored data has a read-only backup overview. Expand **Backup details
and recovery guidance** for allocation, paths, and recovery steps. Do not remove
markers merely to free space. Cache/list maintenance, including the cloud file cache,
uses confirmation; clearing previews does not clear persistent cloud
working copies.

Right-click Wastebasket > **Empty Wastebasket…** opens a permanent-deletion
warning. Emptying cannot be undone and does not empty cloud-provider trash.
Windows network paths use permanent deletion rather than the recycle bin.

On Linux, network-mounted files (including GVFS/SFTP and phone storage) are not
downloaded into local undo backups before deletion. Browsey uses the mounted
backend's trash when supported; recovery is handled by the server/provider,
not Browsey's undo history. If trash is unavailable, a separate confirmation
is required before direct permanent deletion, even when the normal deletion
confirmation is disabled. Cancel leaves the selection unchanged; errors during
execution can leave a partially completed batch and are not automatically
retried. Stop other writers and refresh before deciding whether to retry.
Local files retain their existing undo behavior, including in mixed selections.

Search/duplicate scans skip symlinks, and symlink permissions are not editable.
Extra metadata is loaded when its Properties tab opens. HDR/OpenEXR thumbnail
decoding may take longer than standard raster formats.

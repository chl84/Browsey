# Using Browsey

[Back to README](../README.md) · [Installation](installation.md) ·
[Documentation site](https://chl84.github.io/Browsey/)

## Navigation and shortcuts

Defaults are remappable in Settings:

- `Ctrl+F` search; `Ctrl+G` switch between list and grid.
- `Ctrl+A` select all; `Ctrl+C/X/V` copy, cut, and paste.
- `Ctrl+R` rename; `Ctrl+P` properties.
- `Delete` trash; `Shift+Delete` permanently delete.
- `Ctrl+H` hidden files; `Ctrl+B` bookmarks; `Ctrl+T` terminal.
- `Esc` exits search/filter contexts.

Ctrl + mouse wheel zooms between list and five grid sizes (64, 96, 128, 160,
and 192 CSS pixels). Zoom is window-local and resets on restart. Grid gaps
are 8 px in Cozy and 6 px in Compact.

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
  Copies survive restart and preview-cache clearing. Close other writers before
  **Upload changes as new file**: it checks the original and uses a unique new
  name, never automatically overwriting or uploading the original.
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
Formatting supports exFAT, FAT32, ext4, and btrfs when matching system tools
are installed. Formatting erases the selected device: verify its identity
and keep backups. New ext4/btrfs roots are made writable by the formatting
user; existing volumes are not automatically re-owned.

Progress uses real UDisks percentages when available, otherwise indeterminate
progress. If completion is uncertain, inspect the device before retrying;
Browsey never automatically repeats an erase.

MTP phones are discovered through GIO and mounted on demand. Keep the phone
unlocked with USB file-transfer mode enabled. Temporary I/O errors may require
a connection check and retry. Phone Properties are informational; formatting
and POSIX permission editing are not offered. GVFS/MTP fallback moves without
verifiable output ownership may copy but refuse source deletion; inspect both
sides rather than automatically repeating the move.

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

Search/duplicate scans skip symlinks, and symlink permissions are not editable.
Extra metadata is loaded when its Properties tab opens. HDR/OpenEXR thumbnail
decoding may take longer than standard raster formats.

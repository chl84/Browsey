# Installing Browsey

[Back to README](../README.md) · [User guide](usage.md) · [Development](development.md)

## Platform and runtime requirements

Published packages target Linux x86_64 and require GTK 3 and WebKitGTK 4.1.
RPM packages are provided for Fedora-family systems, and DEB packages for
Ubuntu/Debian-family systems. Package managers resolve declared dependencies.
PDFium and the frontend are bundled; installing a release does not require
Rust, Node.js, npm, or a separate system PDF library.

Recent checks include Arch/Omarchy and an extracted-DEB startup check on Ubuntu
22.04. This is not clean Fedora/Ubuntu installation or upgrade acceptance;
consult the version-specific [release notes](releases/) for validation scope.
Windows is in maintenance mode with no current release installer; macOS is
not supported. Source-build limitations are in the development guide.

## Download and verify

1. Open the release linked from the README.
2. Download the RPM or DEB for your distribution, plus `SHA256SUMS`.
3. In that download folder, verify the selected file. Replace `VERSION` in
   the commands below with the downloaded version number.

For DEB:

```bash
sha256sum --ignore-missing -c SHA256SUMS &&
  sudo apt install ./Browsey_VERSION_amd64.deb
```

For RPM:

```bash
sha256sum --ignore-missing -c SHA256SUMS &&
  sudo dnf install ./Browsey-VERSION-1.x86_64.rpm
```

Confirm that the selected package is reported as `OK` before installation.
Checksums detect damaged or mismatched downloads; the packages are not signed.
`dnf` resolves dependencies, unlike a bare `rpm -Uvh` command. The lower-level
historical package procedure is recorded in
[install/upgrade operations](operations/linux-release/install-upgrade-path.md).

## Upgrade or uninstall

Finish active file operations and close Browsey before upgrading. Install the
newer downloaded package using the same package-manager command. Do not assume
older builds understand newer recovery markers; package downgrade is not a
supported release path.

To uninstall the package, use `sudo apt remove browsey` or
`sudo dnf remove browsey`. User data and retained working/recovery copies need
separate inspection; do not delete them just to reclaim space if they may
contain edits or backups still needed for recovery.

## Optional features

- Cloud storage: rclone 1.67.0 or newer, configured separately with
  `rclone config`. Cloud integration is off by default.
- Video thumbnails: `ffmpeg` in PATH, or set `FFMPEG_BIN`.
- GNOME Wayland file-clipboard interoperability: `xclip` in PATH; the app's
  internal clipboard does not require it.
- MTP phones: a GVFS MTP backend (`gvfs-mtp` on Arch/Fedora, or `gvfs-backends`
  on Ubuntu). Unlock the phone and enable USB file-transfer mode.
- USB formatting: UDisks2, a working PolicyKit authentication agent, and the
  matching filesystem tool: `mkfs.exfat` (`exfatprogs`), `mkfs.fat`
  (`dosfstools`), `mkfs.ext4` (`e2fsprogs`), `mkfs.btrfs` (`btrfs-progs`),
  or `mkntfs` for NTFS. On current Arch Linux, `mkntfs` is provided by
  [`ntfsprogs`](https://archlinux.org/packages/extra/x86_64/ntfsprogs/),
  separately from the `ntfs-3g` mount driver; package names vary by distribution.
  Missing tools are shown as disabled options with an explanation. Browsey also
  checks UDisks formatting support before unmounting or erasing anything.

## User-local installation from source (Linux x86_64)

Install the [build prerequisites](development.md#prerequisites), then run from
the checkout:

```bash
bash scripts/install/install-local.sh --dry-run
bash scripts/install/install-local.sh
```

The installer builds the current checkout with Tauri's production frontend,
checks runtime libraries, and installs under `~/.local/opt/browsey`, with a
launcher at `~/.local/bin/browsey`. Missing frontend dependencies are installed
with `npm ci`. The previous installation is retained in the printed
`.browsey-backup.*` directory alongside it.

After successful verification, it sends a desktop notification using Omarchy's
notification command, with `notify-send` as a fallback on other desktops. No
success notification is sent after a failed install or during `--dry-run`.
Unavailable or unresponsive notification services do not fail the installation;
a warning is printed instead.

It preserves existing desktop integration but does not create/change default
file-manager associations, commit/push, or stop running Browsey processes.
Finish operations before restarting. On a fresh installation, ensure
`~/.local/bin` is on PATH; desktop integration may need separate setup.

The repository's `t3.json` declares an **Install Browsey** project action
running this same script. Where action import is unavailable, add an action
with command `bash scripts/install/install-local.sh`; no special import is
required to run the installer from a terminal.

# Browsey

A clean, keyboard-friendly file explorer, inspired by GNOME Nautilus.
Browsey combines a Svelte interface with a Rust backend, built on Tauri 2.

Downloads: [Browsey 1.0.5 (planned release)](https://github.com/chl84/Browsey/releases/tag/v1.0.5) · [Latest published release](https://github.com/chl84/Browsey/releases/latest).

[Documentation](https://chl84.github.io/Browsey/) · [Installation](docs/installation.md) · [User guide](docs/usage.md) · [Changelog](CHANGELOG.md)

## Features

- Virtualized list and grid views for large folders, thumbnail previews,
  and Ctrl + wheel zoom.
- Recursive search, file filters, stars, bookmarks, and duplicate scanning.
- Copy/move with drag & drop between apps, conflict handling, background
  progress, and local undo/redo.
- ZIP creation and archive extraction, including password-protected ZIP, 7z,
  and RAR files. ZIP creation passwords protect contents, not filenames.
- Advanced batch rename, Open With, Linux default-app selection, and editable
  file permissions.
- System/Omarchy themes, light/dark fallback, and customizable shortcuts.
- USB drive formatting and on-demand MTP phone access on Linux.
- Optional OneDrive, Google Drive, and Nextcloud access through rclone.

## Screenshots

![Browsey showing a Fedora workspace](resources/01_screenshot_browsey_fedora.png)
![Browsey in grid view with thumbnails](resources/02_screenshot_browsey_fedora.png)

## Status

Browsey `1.0.5` is Linux-first. Release preparation is in progress; planned RPM and DEB packages target Linux x86_64. See the [release notes](docs/releases/1.0.5.md) for changes, validation evidence, and outstanding release checks.

- Linux: RPM and DEB packages; GTK 3 and WebKitGTK 4.1 are required.
- Windows: maintenance mode; no new Windows installer is included in 1.0.5.
- macOS: not supported.

## Getting started

Download the package for your distribution from the latest published release above:
RPM for Fedora-family systems, or DEB for Ubuntu/Debian-family systems.
The [installation guide](docs/installation.md) covers dependencies, checksum
verification, installation, upgrades, and the user-local source installer.
Rust and Node.js are needed only when building from source.

Cloud access is off by default. Install rclone, configure a remote with
`rclone config`, and enable it in Settings > Cloud. See the
[cloud guide](docs/usage.md#cloud-storage) for setup and provider limitations.

Common shortcuts: `Ctrl+F` search, `Ctrl+G` list/grid, `Ctrl+R` rename,
`Ctrl+C/X/V` copy/cut/paste, `Delete` trash, and `Ctrl+P` properties.
Shortcuts can be changed in Settings. The [user guide](docs/usage.md) covers
more interactions, file safety, and recovery.

## Development

See [development and release instructions](docs/development.md) for prerequisites,
the pinned Tauri CLI, tests, production builds, and the version-bump routine.
Architecture, audits, and active work are indexed in [project docs](docs/README.md).

## License

[MIT](LICENSE). Bundled component licenses are listed in
[Third-party notices](THIRD_PARTY_NOTICES). The software is provided as-is,
without warranty. Developed with AI assistance from OpenAI Codex.

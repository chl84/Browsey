# Linux Install and Upgrade Path

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Status: Active Linux 1.0 install/upgrade documentation.

## Purpose

Document the supported Linux install and upgrade path for the validated Linux
1.0 target surface.

This is a documentation record only. It does not by itself prove that install,
upgrade, or uninstall behavior has already been validated on clean systems.

## Supported Install Paths

### Fedora Workstation

Primary package format: `.rpm`

Install or replace an existing build:

```bash
sudo rpm -Uvh --replacepkgs Browsey-<version>-1.x86_64.rpm
```

Expected source: GitHub Releases RPM artifact.

### Ubuntu LTS / Debian-family

Primary package format: `.deb`

Install or upgrade via `apt` so dependency resolution stays native to the
distribution:

```bash
sudo apt install ./browsey_<version>_amd64.deb
```

Expected source: GitHub Releases DEB artifact.

## Upgrade Policy

- Supported Linux release path includes install and in-place upgrade.
- The supported package path is native package replacement:
  - `rpm -Uvh --replacepkgs ...` on Fedora
  - `apt install ./browsey_<version>_amd64.deb` on Ubuntu/Debian-family
- Package downgrade is not part of the supported Linux 1.0 release path.

## Notes

- This document intentionally does not define uninstall semantics yet; that is
  still a separate Step 9 item.
- This document also does not claim that installed-app launch, desktop entry
  behavior, file associations, or upgrade-state migration have already been
  validated.

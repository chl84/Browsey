# Linux 1.0 Packaging Plan

Created: 2026-03-06
Track: `docs/todo-archive/TODO_PRODUCTION_READY_LINUX.md`
Status: Linux 1.0 packaging decision record; validation below is scoped to that track.

## Purpose

Record the package/distribution format decisions for the supported Linux 1.0
target surface so Step 9 can be validated against a stable install story.

## Target Surface

- Fedora Workstation: primary Linux validation target
- Ubuntu LTS desktop: required Debian-family validation target

## Packaging Decision

Browsey Linux 1.0 will ship these primary release artifacts:

- Fedora target: Tauri-generated `.rpm`
- Ubuntu/Debian-family target: Tauri-generated `.deb`

This keeps the supported install path aligned with native package tooling on
the validated Linux targets instead of introducing a parallel installer format
for Linux 1.0.

## Build Path

- Repository bundle targets: `rpm`, `deb`
- Helper script: `scripts/build/build-release.sh`
- Expected output roots:
  - `target/release/bundle/rpm/`
  - `target/release/bundle/deb/`

## Upgrade and Downgrade Policy

- In-place upgrade is part of the Linux 1.0 validation scope.
- Package downgrade is not a supported Linux 1.0 workflow.

If a downgrade is attempted, it is outside the supported release path and
should not be treated as a Linux 1.0 release guarantee.

## What This Decision Does Not Yet Prove

This document decides the supported package format only. It does not by itself
prove:

- clean install/upgrade behavior on Fedora
- clean install/upgrade behavior on Ubuntu/Debian
- uninstall behavior on the supported targets
- desktop entry/file association correctness in installed builds

These were separate Step 9 validation items, with Linux 1.0 completion recorded
in the [RC log](release-candidate-log.md) and archived track. New releases need
their own packaging evidence in [release notes](../../releases/); choosing RPM
and DEB alone does not validate installation on a target distribution.

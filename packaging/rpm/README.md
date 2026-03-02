# RPM Spec Usage

`browsey.spec` in this directory is an optional packaging path for manual
`rpmbuild`/COPR workflows.

It is **not** used by the default Browsey release flow:

- local/release scripts use `cargo tauri build --bundles rpm`
- GitHub workflows do not consume this spec file

Use this spec only when you intentionally build/package Browsey with
`rpmbuild` (for example COPR).

## Before Using the Spec

1. Update `Version` in `browsey.spec` to match `Cargo.toml`.
2. Update `%changelog` for the release.
3. Build a source archive named `browsey-<version>.tar.gz` expected by `Source0`.

## Typical Manual Build

```bash
rpmbuild -ba packaging/rpm/browsey.spec \
  --define "_sourcedir $(pwd)" \
  --define "_topdir $(pwd)/.rpmbuild"
```

Adjust `rpmbuild` paths/macros for your environment or COPR setup.

# Updating bundled PDFium

Browsey ships PDFium **156.0.8076.0** for Linux x64 and Windows x64. It uses
`pdfium-render` **0.9.4** with explicit `pdfium_7881`, `thread_safe`, and
`image_025` features. Build 8076 satisfies this API profile in the Linux
regressions; the Windows DLL exports all 465 public PDFium/Form functions
exported by the Linux library, plus two Windows-only functions. Windows runtime
rendering still requires a Windows smoke test.

Both distributions have V8 and XFA disabled. Do not substitute a V8/XFA package
or change API profiles implicitly through `pdfium_latest`.

## Provenance and integrity

`resources/pdfium.json` records the immutable release, distribution commit,
archive URLs and SHA-256 hashes, installed binary hashes, and binding profile.
`THIRD_PARTY_NOTICES` distinguishes PDFium's BSD-style license from the
distributor's MIT license and the wrapper's MIT OR Apache-2.0 license declaration.
Headers, version/configuration files, and dependency licenses are copied from
the same upstream archives as the binaries. Windows license files retain
upstream CRLF line endings; the integrity check compares their text with Linux
after normalizing line endings only for comparison.

Run:

```sh
node scripts/maintenance/check-pdfium.mjs
cargo test --locked pdfium_
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
```

The integrity check runs in the Rust quality and Linux release workflows.
PDF regression tests require the repository's native library and do not silently
skip if loading or binding fails. They cover repeated use, simultaneous
metadata/thumbnails, rotations, three standard fonts, narrow pages, malformed
documents, and an upstream encrypted fixture with public test passwords.
Tests do not add password prompts for PDF thumbnails or properties: locked PDFs
continue to fail thumbnail generation cleanly and return no PDF metadata.

## Update procedure

1. Select a fixed release and the regular `pdfium-linux-x64.tgz` and
   `pdfium-win-x64.tgz` assets. Compare each downloaded archive's SHA-256 with
   the digest published by the GitHub release API before extracting anything.
2. Inspect archive paths, then refresh each vendored directory as a unit:
   binary/import library, headers, `VERSION`, `PDFiumConfig.cmake`, `args.gn`,
   `LICENSE`, and `licenses/`. Remove only licenses absent from the new package;
   keep Tauri's existing Linux sidecar symlink.
3. Pin the Rust wrapper and a named supported API profile in `Cargo.toml`,
   update only that dependency in `Cargo.lock`, and adapt fallible APIs.
4. Update the manifest, notices, current README, and Unreleased changelog.
   Do not rewrite historical release notes. Verify native binding at runtime;
   compilation alone does not prove that all requested symbols are available.
5. Run the checks above, then build via Tauri, not plain `cargo build --release`:
   `frontend/node_modules/.bin/tauri build --no-bundle -- --locked`.
6. Back up the old executable and library. Install both new files, plus notices
   and packaged license resources. Do not overwrite a mapped shared library in
   place: use a staged file and atomic rename, then restart the application when
   no file operation is active. Retain backups for rollback.

## Runtime ownership

`src/pdfium_runtime.rs` owns one process-lifetime PDFium instance initialized
through a fallible `OnceCell`. Thumbnails and metadata share it. Failed
initialization may be retried; successful initialization is never repeated.
The wrapper's native-call mutex remains enabled. An explicit `PDFIUM_LIB_PATH`
override is applied only before first successful initialization, followed by
bundled and system fallbacks. A restart is required to change libraries.

# TODO Backend Error API Remaining

Scope: remaining backend areas that still rely on raw `Result<_, String>` or equivalent stringly typed internal error flow instead of a typed Browsey error model that maps to `ApiError` only at the command surface.

Not in scope:
- pure formatting helpers that return `String`
- low-level transport helpers that already return a typed non-Browsey internal error (`RcloneCliError`, DB error, etc.)

## Remaining Cases

- [ ] `src/commands/decompress/`
  - `mod.rs`
  - `util.rs`
  - `zip_format.rs`
  - `rar_format.rs`
  - `seven_z_format.rs`
  - `tar_format.rs`
  - Remaining archive/IO helpers still return `Result<_, String>` and are only wrapped later via `map_external_result(...)`.

- [ ] `src/commands/compress/mod.rs`
  - Archive planning/writing still has raw string errors in internal helpers.
  - Remaining cases include archive writer and copy/finalize paths.

- [ ] `src/commands/thumbnails/`
  - `mod.rs`
  - `thumbnails_pdf.rs`
  - `thumbnails_svg.rs`
  - `thumbnails_video.rs`
  - The orchestration now uses `ThumbnailResult`, but several decode/render helpers still return `Result<_, String>`.

- [ ] `src/commands/system_clipboard/mod.rs`
  - Clipboard subprocess helpers still use `Result<_, String>` for wl-copy/xclip/read/clear paths.

- [ ] `src/commands/open_with/`
  - `mod.rs`
  - `linux.rs`
  - `windows.rs`
  - Launcher and command-template helpers still use `Result<_, String>`.

- [ ] `src/commands/duplicates/`
  - `mod.rs`
  - `scan.rs`
  - Scan planning and synchronous execution still use raw string errors internally.

- [ ] `src/commands/listing/mod.rs`
  - Core synchronous listing/facets helpers still expose `Result<_, String>` in internal seams.

- [ ] `src/commands/cloud/providers/rclone/`
  - `parse.rs`
  - `read.rs`
  - `remotes.rs`
  - Cloud command surfaces now use typed errors, but several provider/parser helpers still return `Result<_, String>` and are mapped later.

- [ ] `src/commands/fs/`
  - `mod.rs`
  - `open_ops.rs`
  - `windows.rs`
  - `trash/staging.rs`
  - A few filesystem helpers still return raw string errors instead of `FsError`/`FsResult`.

## Suggested Order

- [ ] `cloud/providers/rclone`
- [ ] `system_clipboard`
- [ ] `open_with`
- [ ] `listing`
- [ ] `duplicates`
- [ ] `thumbnails`
- [ ] `compress`
- [ ] `decompress`
- [ ] `fs` residual helpers

## Quality Gates

- [ ] each touched module exposes a typed internal `...Result<T>`
- [ ] `ApiError` mapping remains only at command surface
- [ ] no new `Result<_, String>` is introduced in touched paths
- [ ] `cargo check -q`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`

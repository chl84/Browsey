# TODO Backend Error API Remaining

Scope: remaining backend areas that still rely on raw `Result<_, String>` or equivalent stringly typed internal error flow instead of a typed Browsey error model that maps to `ApiError` only at the command surface.

Not in scope:
- pure formatting helpers that return `String`
- low-level transport helpers that already return a typed non-Browsey internal error (`RcloneCliError`, DB error, etc.)

## Remaining Cases

- [x] `src/commands/decompress/`
  - `mod.rs`
  - `util.rs`
  - `zip_format.rs`
  - `rar_format.rs`
  - `seven_z_format.rs`
  - `tar_format.rs`
  - Completed: archive/IO helpers now use `DecompressResult<_>` through the remaining extraction/file-path seams.

- [x] `src/commands/compress/mod.rs`
  - Completed: archive planning/writing now uses `CompressResult<_>` through the remaining writer/copy seams.

- [x] `src/commands/thumbnails/`
  - `mod.rs`
  - `thumbnails_pdf.rs`
  - `thumbnails_svg.rs`
  - `thumbnails_video.rs`
  - Completed: render/decode helpers now use `ThumbnailResult<_>` throughout the module.

- [x] `src/commands/system_clipboard/mod.rs`
  - Completed: clipboard subprocess helpers now use `SystemClipboardResult<_>` instead of `Result<_, String>`.

- [x] `src/commands/open_with/`
  - `mod.rs`
  - `linux.rs`
  - `windows.rs`
  - Completed: launcher and command-template helpers now use `OpenWithResult<_>`.

- [x] `src/commands/duplicates/`
  - `mod.rs`
  - `scan.rs`
  - Completed: scan planning and synchronous execution now use `DuplicatesResult<_>` and typed scan aborts.

- [x] `src/commands/listing/mod.rs`
  - Completed: core synchronous listing/facets helpers now use `ListingResult<_>` across their internal seams.

- [x] `src/commands/cloud/providers/rclone/`
  - `parse.rs`
  - `read.rs`
  - `remotes.rs`
  - Completed: provider/parser helpers now use typed internal errors instead of `Result<_, String>`.

- [ ] `src/commands/fs/`
  - `mod.rs`
  - `open_ops.rs`
  - `windows.rs`
  - `trash/staging.rs`
  - Progress: `mod.rs` and `open_ops.rs` now expose typed `FsResult<_>` seams for path expansion/open helpers.
  - Remaining: `windows.rs` and `trash/staging.rs` still use raw string errors in internal helpers.

## Suggested Order

- [x] `cloud/providers/rclone`
- [x] `system_clipboard`
- [x] `open_with`
- [x] `listing`
- [x] `duplicates`
- [x] `thumbnails`
- [x] `compress`
- [x] `decompress`
- [ ] `fs` residual helpers

## Quality Gates

- [x] each touched module exposes a typed internal `...Result<T>`
- [x] `ApiError` mapping remains only at command surface
- [x] no new `Result<_, String>` is introduced in touched paths
- [x] `cargo check -q`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`

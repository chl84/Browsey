# TODO: Error API Migration

Created: 2026-02-18
Goal: Migrate remaining modules from `Result<..., String>` to the new code-based backend error flow (`ApiError` + domain error codes).

## Commands modules

- [x] `src/commands/bookmarks.rs`
- [x] `src/commands/compress`
- [x] `src/commands/console.rs`
- [x] `src/commands/decompress`
- [x] `src/commands/duplicates`
- [x] `src/commands/entry_metadata`
- [x] `src/commands/fs`
- [x] `src/commands/keymap.rs`
- [x] `src/commands/library.rs`
- [x] `src/commands/listing`
- [x] `src/commands/network`
- [x] `src/commands/open_with`
- [x] `src/commands/permissions`
- [x] `src/commands/search`
- [x] `src/commands/settings`
- [x] `src/commands/system_clipboard`
- [x] `src/commands/thumbnails`

## Core modules outside `commands`

- [x] `src/clipboard`
- [x] `src/db`
- [x] `src/entry`
- [ ] `src/fs_utils`
- [x] `src/keymap`
- [ ] `src/metadata`
- [x] `src/path_guard`
- [ ] `src/statusbar`
- [x] `src/tasks`
- [ ] `src/undo`
- [ ] `src/watcher.rs`

## Already on new flow (reference)

- [x] `src/errors/api_error.rs`
- [x] `src/errors/domain.rs`
- [x] `src/commands/rename`

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
- [ ] `src/commands/fs` (partially migrated: `set_hidden` uses new flow)
- [x] `src/commands/keymap.rs`
- [x] `src/commands/library.rs`
- [x] `src/commands/listing`
- [ ] `src/commands/network`
- [ ] `src/commands/open_with`
- [ ] `src/commands/permissions` (partially migrated; some internal helpers still return `String`)
- [ ] `src/commands/search`
- [ ] `src/commands/settings`
- [ ] `src/commands/system_clipboard`
- [ ] `src/commands/thumbnails`

## Core modules outside `commands`

- [ ] `src/clipboard`
- [ ] `src/db`
- [ ] `src/entry`
- [ ] `src/fs_utils`
- [ ] `src/keymap`
- [ ] `src/metadata`
- [ ] `src/path_guard`
- [ ] `src/statusbar`
- [ ] `src/tasks`
- [ ] `src/undo`
- [ ] `src/watcher.rs`

## Already on new flow (reference)

- [x] `src/errors/api_error.rs`
- [x] `src/errors/domain.rs`
- [x] `src/commands/rename`

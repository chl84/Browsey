export {
  DEFAULT_SHORTCUTS,
  keyboardEventToAccelerator,
  eventMatchesAccelerator,
  matchesShortcut,
  matchesAnyShortcut,
  shortcutFor,
} from './keymap'
export type { ShortcutBinding, ShortcutCommandId } from './keymap'
export {
  loadShortcuts,
  setShortcutBinding,
  resetShortcutBinding,
  resetAllShortcuts,
} from './service'

import { invoke } from '@/lib/tauri'
import type { ShortcutBinding, ShortcutCommandId } from './keymap'

export const loadShortcuts = () => invoke<ShortcutBinding[]>('load_shortcuts')

export const setShortcutBinding = (
  commandId: ShortcutCommandId,
  accelerator: string,
) => invoke<ShortcutBinding[]>('set_shortcut_binding', { commandId, accelerator })

export const resetShortcutBinding = (commandId: ShortcutCommandId) =>
  invoke<ShortcutBinding[]>('reset_shortcut_binding', { commandId })

export const resetAllShortcuts = () =>
  invoke<ShortcutBinding[]>('reset_all_shortcuts')

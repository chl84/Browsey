import { invoke } from '@/lib/tauri'

export const undoAction = () =>
  invoke<void>('undo_action')

export const redoAction = () =>
  invoke<void>('redo_action')

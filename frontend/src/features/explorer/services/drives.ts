import { invoke } from '@tauri-apps/api/core'

export const ejectDrive = (path: string) =>
  invoke<void>('eject_drive', { path })

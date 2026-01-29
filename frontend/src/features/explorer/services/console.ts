import { invoke } from '@tauri-apps/api/core'

export const openConsole = (path: string) =>
  invoke<void>('open_console', { path })

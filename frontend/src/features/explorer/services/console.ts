import { invoke } from '@/lib/tauri'

export const openConsole = (path: string) =>
  invoke<void>('open_console', { path })

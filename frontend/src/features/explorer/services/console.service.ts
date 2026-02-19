import { invoke } from '@/shared/lib/tauri'

export const openConsole = (path: string) =>
  invoke<void>('open_console', { path })

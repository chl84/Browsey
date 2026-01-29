import { invoke } from '@tauri-apps/api/core'

export const toggleStar = (path: string) =>
  invoke<boolean>('toggle_star', { path })

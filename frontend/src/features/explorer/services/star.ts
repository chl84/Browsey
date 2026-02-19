import { invoke } from '@/lib/tauri'

export const toggleStar = (path: string) =>
  invoke<boolean>('toggle_star', { path })

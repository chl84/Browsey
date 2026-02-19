import { invoke } from '@/shared/lib/tauri'

export const toggleStar = (path: string) =>
  invoke<boolean>('toggle_star', { path })

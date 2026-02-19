import { invoke } from '@/shared/lib/tauri'

export const cancelTask = (id: string) =>
  invoke<void>('cancel_task', { id })

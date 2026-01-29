import { invoke } from '@tauri-apps/api/core'

export const cancelTask = (id: string) =>
  invoke<void>('cancel_task', { id })

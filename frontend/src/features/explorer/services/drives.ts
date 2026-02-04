import { invoke } from '@tauri-apps/api/core'

export const ejectDrive = (path: string) => invoke<void>('eject_drive', { path })

export const mountPartition = (path: string) => invoke<void>('mount_partition', { path })

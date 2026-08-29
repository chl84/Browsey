import { invoke } from '@/shared/lib/tauri'

export const ejectDrive = (path: string) => invoke<void>('eject_drive', { path })

export const formatRemovablePartition = (path: string) =>
  invoke<void>('format_removable_partition', { path })

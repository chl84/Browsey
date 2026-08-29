import { invoke } from '@/shared/lib/tauri'

export const ejectDrive = (path: string) => invoke<void>('eject_drive', { path })

export const formatRemovablePartition = (
  path: string,
  filesystem: 'exfat' | 'fat32' | 'ext4' | 'btrfs',
) => invoke<void>('format_removable_partition', { path, filesystem })

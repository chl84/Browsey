import { invoke } from '@/shared/lib/tauri'

export type UsbFilesystemOption = {
  id: 'exfat' | 'fat32' | 'ext4' | 'btrfs'
  label: string
  description: string
  available: boolean
}

export type UsbFormatInfo = {
  device: string
  model: string
  sizeBytes: number
  filesystems: UsbFilesystemOption[]
}

export type UsbFormatResult = {
  device: string
  mountPath: string | null
  sizeBytes: number
  filesystem: string
  label: string | null
}

export const ejectDrive = (path: string) => invoke<void>('eject_drive', { path })

export const formatRemovablePartition = (
  path: string,
  filesystem: 'exfat' | 'fat32' | 'ext4' | 'btrfs',
  label: string,
) => invoke<UsbFormatResult>('format_removable_partition', { path, filesystem, label })

export const getRemovableUsbFormatInfo = (path: string) =>
  invoke<UsbFormatInfo>('get_removable_usb_format_info', { path })

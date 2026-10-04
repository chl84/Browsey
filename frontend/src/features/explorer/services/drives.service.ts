import { invoke } from '@/shared/lib/tauri'
import { Channel } from '@tauri-apps/api/core'
import type { Partition } from '../model/types'

export type UsbFormatProgress = { phase: string; percent: number | null }
export type UsbFilesystem = 'exfat' | 'fat32' | 'ext4' | 'btrfs' | 'ntfs'

export type UsbFilesystemOption = {
  id: UsbFilesystem
  label: string
  description: string
  available: boolean
  requiredTool: string
  labelMaxLength: number
}

export const usbVolumeLabelError = (label: string, option: UsbFilesystemOption | undefined) => {
  if (!option) return ''
  const value = label.trim()
  return value.length > option.labelMaxLength || !/^[A-Za-z0-9 _-]*$/.test(value)
    ? `Use up to ${option.labelMaxLength} ASCII letters, numbers, spaces, hyphens, or underscores.`
    : ''
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

export type VolumeUsage = {
  totalBytes: number
  usedBytes: number
  freeBytes: number
  reservedBytes: number
}

export const getVolumeUsage = (path: string) =>
  invoke<VolumeUsage | null>('get_volume_usage', { path })

export const isUnmountedUsb = (path: string) => path.startsWith('usb-volume://')
export const isMtpUri = (path: string) => /^mtp:\/\//i.test(path)
export const isMtpPartition = (part: Partition) =>
  part.fs?.toLowerCase() === 'mtp' || isMtpUri(part.path) || part.path.includes('/gvfs/mtp:')
export const isUnmountedPartition = (path: string) => isUnmountedUsb(path) || isMtpUri(path)
export const canFormatPartition = (part: Partition) =>
  part.removable === true && !isMtpPartition(part) && !part.path.includes('/gvfs/') &&
  (!part.path.includes('://') || isUnmountedUsb(part.path))
export const mountUsbVolume = (path: string) => invoke<string>('mount_usb_volume', { path })

export const formatRemovablePartition = async (
  path: string,
  filesystem: UsbFilesystem,
  label: string,
  onProgress: (progress: UsbFormatProgress) => void = () => {},
) => {
  let active = true
  const channel = new Channel<UsbFormatProgress>()
  channel.onmessage = (progress) => { if (active) onProgress(progress) }
  try {
    return await invoke<UsbFormatResult>('format_removable_partition', { path, filesystem, label, onProgress: channel })
  } finally {
    active = false
  }
}

export const getRemovableUsbFormatInfo = (path: string) =>
  invoke<UsbFormatInfo>('get_removable_usb_format_info', { path })

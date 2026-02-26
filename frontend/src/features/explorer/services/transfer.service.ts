import { invoke } from '@/shared/lib/tauri'

export type MixedTransferConflictInfo = {
  src: string
  target: string
  exists: boolean
  isDir: boolean
}

export type MixedTransferWriteOptions = {
  overwrite?: boolean
  prechecked?: boolean
}

export const previewMixedTransferConflicts = (sources: string[], destDir: string) =>
  invoke<MixedTransferConflictInfo[]>('preview_mixed_transfer_conflicts', { sources, destDir })

export const copyMixedEntries = (
  sources: string[],
  destDir: string,
  options?: MixedTransferWriteOptions,
) =>
  invoke<string[]>('copy_mixed_entries', {
    sources,
    destDir,
    overwrite: options?.overwrite ?? false,
    prechecked: options?.prechecked ?? false,
  })

export const moveMixedEntries = (
  sources: string[],
  destDir: string,
  options?: MixedTransferWriteOptions,
) =>
  invoke<string[]>('move_mixed_entries', {
    sources,
    destDir,
    overwrite: options?.overwrite ?? false,
    prechecked: options?.prechecked ?? false,
  })

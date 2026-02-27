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
  progressEvent?: string
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
    progressEvent: options?.progressEvent,
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
    progressEvent: options?.progressEvent,
  })

export const copyMixedEntryTo = (
  src: string,
  dst: string,
  options?: MixedTransferWriteOptions,
) =>
  invoke<string>('copy_mixed_entry_to', {
    src,
    dst,
    overwrite: options?.overwrite ?? false,
    prechecked: options?.prechecked ?? false,
    progressEvent: options?.progressEvent,
  })

export const moveMixedEntryTo = (
  src: string,
  dst: string,
  options?: MixedTransferWriteOptions,
) =>
  invoke<string>('move_mixed_entry_to', {
    src,
    dst,
    overwrite: options?.overwrite ?? false,
    prechecked: options?.prechecked ?? false,
    progressEvent: options?.progressEvent,
  })

import { invoke } from '@/shared/lib/tauri'

export type MixedTransferConflictInfo = {
  src: string
  target: string
  exists: boolean
  isDir: boolean
}

export const previewMixedTransferConflicts = (sources: string[], destDir: string) =>
  invoke<MixedTransferConflictInfo[]>('preview_mixed_transfer_conflicts', { sources, destDir })


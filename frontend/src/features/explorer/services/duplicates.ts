import { invoke } from '@tauri-apps/api/core'

export type DuplicateScanPhase = 'collecting' | 'comparing' | 'done'

export type DuplicateScanProgress = {
  phase: DuplicateScanPhase
  percent: number
  scannedFiles: number
  candidateFiles: number
  comparedFiles: number
  matchedFiles: number
  done: boolean
  error?: string | null
  duplicates?: string[] | null
}

export const checkDuplicatesStream = (args: {
  targetPath: string
  startPath: string
  progressEvent: string
}) => invoke<void>('check_duplicates_stream', args)

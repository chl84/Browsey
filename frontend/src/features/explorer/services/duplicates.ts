import { invoke } from '@tauri-apps/api/core'

export const checkDuplicates = (targetPath: string, startPath: string) =>
  invoke<string[]>('check_duplicates', { targetPath, startPath })

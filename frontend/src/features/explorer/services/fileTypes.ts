import { invoke } from '@tauri-apps/api/core'

export type NewFileTypeMatch = {
  label: string
  mime: string
  matchedExt: string | null
}

export const detectNewFileType = (name: string) =>
  invoke<NewFileTypeMatch | null>('detect_new_file_type', { name })

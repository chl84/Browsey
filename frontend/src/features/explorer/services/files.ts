import { invoke } from '@/lib/tauri'
import type { Entry } from '../types'

export const openEntry = (entry: Entry) =>
  invoke<void>('open_entry', { path: entry.path })

export const renameEntry = (path: string, newName: string) =>
  invoke<string>('rename_entry', { path, newName })

export const renameEntries = (entries: Array<{ path: string; newName: string }>) =>
  invoke<string[]>('rename_entries', { entries })

export type AdvancedRenamePreviewPayload = {
  regex: string
  replacement: string
  prefix: string
  suffix: string
  caseSensitive: boolean
  sequenceMode: 'none' | 'numeric' | 'alpha'
  sequencePlacement: 'start' | 'end'
  sequenceStart: number
  sequenceStep: number
  sequencePad: number
}

export type AdvancedRenamePreviewRow = {
  original: string
  next: string
}

export type AdvancedRenamePreviewResult = {
  rows: AdvancedRenamePreviewRow[]
  error?: string | null
}

export const previewRenameEntries = (
  entries: Array<{ path: string; name: string }>,
  payload: AdvancedRenamePreviewPayload,
) => invoke<AdvancedRenamePreviewResult>('preview_rename_entries', { entries, payload })

export const createFolder = (base: string, name: string) =>
  invoke<string>('create_folder', { path: base, name })

export const createFile = (base: string, name: string) =>
  invoke<string>('create_file', { path: base, name })

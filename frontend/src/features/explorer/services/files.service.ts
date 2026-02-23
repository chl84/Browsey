import { invoke } from '@/shared/lib/tauri'
import type { Entry } from '../model/types'

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
  keepExtension: boolean
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

export type EntryKind = 'dir' | 'file'

export const entryKind = (path: string) =>
  invoke<EntryKind>('entry_kind_cmd', { path })

export const dirSizes = (paths: string[], progressEvent?: string) =>
  invoke<{ total: number; total_items: number }>('dir_sizes', { paths, progressEvent })

export const canExtractPaths = (paths: string[]) =>
  invoke<boolean>('can_extract_paths', { paths })

export type ExtractResult = {
  destination: string
  skipped_symlinks: number
  skipped_entries: number
}

export type ExtractBatchItem = {
  path: string
  ok: boolean
  result?: ExtractResult | null
  error?: string | null
}

export const extractArchive = (path: string, progressEvent?: string) =>
  invoke<ExtractResult>('extract_archive', { path, progressEvent })

export const extractArchives = (paths: string[], progressEvent?: string) =>
  invoke<ExtractBatchItem[]>('extract_archives', { paths, progressEvent })

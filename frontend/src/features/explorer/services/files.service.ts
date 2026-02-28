import { invoke } from '@/shared/lib/tauri'
import { createCloudFolder, renameCloudEntry } from '@/features/network'
import type { Entry } from '../model/types'

const isCloudPath = (path: string) => path.startsWith('rclone://')

const joinCloudPath = (dir: string, name: string) => `${dir.replace(/\/+$/, '')}/${name}`

const parentCloudPath = (path: string) => {
  const idx = path.lastIndexOf('/')
  return idx > 'rclone://'.length ? path.slice(0, idx) : path
}

export const openEntry = (entry: Entry) => {
  if (isCloudPath(entry.path) && entry.kind !== 'dir') {
    return invoke<void>('open_cloud_entry', { path: entry.path })
  }
  return invoke<void>('open_entry', { path: entry.path })
}

export const renameEntry = async (path: string, newName: string) => {
  if (!isCloudPath(path)) {
    return invoke<string>('rename_entry', { path, newName })
  }
  const dst = joinCloudPath(parentCloudPath(path), newName)
  await renameCloudEntry(path, dst, { overwrite: false })
  return dst
}

export const renameEntries = (entries: Array<{ path: string; newName: string }>) => {
  if (entries.some((entry) => isCloudPath(entry.path))) {
    throw new Error('Advanced rename is not supported for cloud entries yet')
  }
  return invoke<string[]>('rename_entries', { entries })
}

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
) => {
  if (entries.some((entry) => isCloudPath(entry.path))) {
    throw new Error('Advanced rename preview is not supported for cloud entries yet')
  }
  return invoke<AdvancedRenamePreviewResult>('preview_rename_entries', { entries, payload })
}

export const createFolder = async (base: string, name: string) => {
  if (!isCloudPath(base)) {
    return invoke<string>('create_folder', { path: base, name })
  }
  const created = joinCloudPath(base, name)
  await createCloudFolder(created)
  return created
}

export const createFile = (base: string, name: string) => {
  if (isCloudPath(base)) {
    throw new Error('Creating files directly in cloud folders is not supported yet')
  }
  return invoke<string>('create_file', { path: base, name })
}

export type EntryKind = 'dir' | 'file'

export const entryKind = (path: string) => {
  if (isCloudPath(path)) {
    throw new Error('Cloud entry type probing is not supported via local entryKind')
  }
  return invoke<EntryKind>('entry_kind_cmd', { path })
}

export const dirSizes = (paths: string[], progressEvent?: string) => {
  if (paths.some(isCloudPath)) {
    throw new Error('Directory size scan is not supported for cloud entries yet')
  }
  return invoke<{ total: number; total_items: number }>('dir_sizes', { paths, progressEvent })
}

export const canExtractPaths = (paths: string[]) => {
  if (paths.some(isCloudPath)) {
    return Promise.resolve(false)
  }
  return invoke<boolean>('can_extract_paths', { paths })
}

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

export const extractArchive = (path: string, progressEvent?: string) => {
  if (isCloudPath(path)) {
    throw new Error('Archive extraction is not supported for cloud entries yet')
  }
  return invoke<ExtractResult>('extract_archive', { path, progressEvent })
}

export const extractArchives = (paths: string[], progressEvent?: string) => {
  if (paths.some(isCloudPath)) {
    throw new Error('Archive extraction is not supported for cloud entries yet')
  }
  return invoke<ExtractBatchItem[]>('extract_archives', { paths, progressEvent })
}

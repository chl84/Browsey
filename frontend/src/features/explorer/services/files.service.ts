import { cloudParentPath, joinCloudPath } from '../cloudPaths'
import { invoke } from '@/shared/lib/tauri'
import { normalizeError } from '@/shared/lib/error'
import { createCloudFolder, openCloudEntry, renameCloudEntry } from '@/features/network'
import type { Entry } from '../model/types'
import { homeDir } from '@tauri-apps/api/path'

export const getHomeDirectory = () => homeDir()

const isCloudPath = (path: string) => path.startsWith('rclone://')


const entryName = (name: string, kind: 'file' | 'folder' | 'entry') => {
  const trimmed = name.trim()
  if (!trimmed || /[/\\\0]/.test(trimmed) || trimmed === '.' || trimmed === '..') {
    throw new Error(`Invalid ${kind} name`)
  }
  return name
}

export const openEntry = (entry: Entry, options?: { progressEvent?: string }) => {
  if (isCloudPath(entry.path) && entry.kind !== 'dir') {
    return openCloudEntry(entry.path, options?.progressEvent)
  }
  return invoke<void>('open_entry', { path: entry.path })
}

export const renameEntry = async (path: string, newName: string) => {
  newName = entryName(newName, 'entry')
  if (!isCloudPath(path)) {
    return invoke<string>('rename_entry', { path, newName })
  }
  const dst = joinCloudPath(cloudParentPath(path), newName)
  await renameCloudEntry(path, dst, { overwrite: false })
  return dst
}

export const renameEntries = async (entries: Array<{ path: string; newName: string }>) => {
  entries = entries.map(entry => ({ ...entry, newName: entryName(entry.newName, 'entry') }))
  if (entries.some((entry) => isCloudPath(entry.path))) {
    if (!entries.every((entry) => isCloudPath(entry.path))) throw new Error('Rename local and cloud entries separately')
    const result = await invoke<{ renamed: string[]; error: string | null }>('rename_cloud_entries', { entries })
    if (result.error) throw new Error(result.error)
    return result.renamed
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
  return invoke<AdvancedRenamePreviewResult>('preview_rename_entries', { entries, payload })
}

export const createFolder = async (base: string, name: string) => {
  const leaf = entryName(name, 'folder')
  if (!isCloudPath(base)) {
    return invoke<string>('create_folder', { path: base, name: leaf })
  }
  const created = joinCloudPath(base, leaf)
  await createCloudFolder(created)
  return created
}

export const createFile = (base: string, name: string) => {
  const leaf = entryName(name, 'file')
  if (isCloudPath(base)) {
    return invoke<string>('create_cloud_file', { path: joinCloudPath(base, leaf) })
  }
  return invoke<string>('create_file', { path: base, name: leaf })
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
  error_code?: string | null
}

export const extractArchive = (path: string, progressEvent?: string, password?: string) => {
  if (isCloudPath(path)) {
    return invoke<ExtractResult>('extract_cloud_archive', { path, progressEvent, ...(password === undefined ? {} : { password }) })
  }
  return invoke<ExtractResult>('extract_archive', { path, progressEvent, ...(password === undefined ? {} : { password }) })
}

export const extractArchives = async (paths: string[], progressEvent?: string) => {
  if (paths.some(isCloudPath)) {
    if (!paths.every(isCloudPath)) throw new Error('Extract local and cloud archives separately')
    const results: ExtractBatchItem[] = []
    for (const path of paths) {
      try { results.push({ path, ok: true, result: await extractArchive(path, progressEvent) }) }
      catch (error) {
        const normalized = normalizeError(error)
        results.push({ path, ok: false, error: normalized.message, error_code: normalized.code })
        if (normalized.code === 'cancelled') break
      }
    }
    return results
  }
  return invoke<ExtractBatchItem[]>('extract_archives', { paths, progressEvent })
}

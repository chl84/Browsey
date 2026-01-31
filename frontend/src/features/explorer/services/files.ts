import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'

export const openEntry = (entry: Entry) =>
  invoke<void>('open_entry', { path: entry.path })

export const renameEntry = (path: string, newName: string) =>
  invoke<void>('rename_entry', { path, newName })

export const createFolder = (base: string, name: string) =>
  invoke<string>('create_folder', { path: base, name })

export const createFile = (base: string, name: string) =>
  invoke<string>('create_file', { path: base, name })

import { invoke } from '@/shared/lib/tauri'

export type CloudProviderKind = 'onedrive' | 'gdrive' | 'nextcloud'
export type CloudEntryKind = 'file' | 'dir'

export type CloudCapabilities = {
  canList: boolean
  canMkdir: boolean
  canDelete: boolean
  canRename: boolean
  canMove: boolean
  canCopy: boolean
  canTrash: boolean
  canUndo: boolean
  canPermissions: boolean
}

export type CloudRemote = {
  id: string
  label: string
  provider: CloudProviderKind
  rootPath: string
  capabilities: CloudCapabilities
}

export type CloudEntry = {
  name: string
  path: string
  kind: CloudEntryKind
  size: number | null
  modified: string | null
  capabilities: CloudCapabilities
}

export type CloudConflictInfo = {
  src: string
  target: string
  exists: boolean
  isDir: boolean
}

export const listCloudRemotes = () =>
  invoke<CloudRemote[]>('list_cloud_remotes')

export const listCloudEntries = (path: string) =>
  invoke<CloudEntry[]>('list_cloud_entries', { path })

export const statCloudEntry = (path: string) =>
  invoke<CloudEntry | null>('stat_cloud_entry', { path })

export const normalizeCloudPath = (path: string) =>
  invoke<string>('normalize_cloud_path', { path })

export const createCloudFolder = (path: string) =>
  invoke<void>('create_cloud_folder', { path })

export const deleteCloudFile = (path: string) =>
  invoke<void>('delete_cloud_file', { path })

export const deleteCloudDirRecursive = (path: string) =>
  invoke<void>('delete_cloud_dir_recursive', { path })

export const deleteCloudDirEmpty = (path: string) =>
  invoke<void>('delete_cloud_dir_empty', { path })

export const moveCloudEntry = (src: string, dst: string) =>
  invoke<void>('move_cloud_entry', { src, dst })

export const renameCloudEntry = (src: string, dst: string) =>
  invoke<void>('rename_cloud_entry', { src, dst })

export const copyCloudEntry = (src: string, dst: string) =>
  invoke<void>('copy_cloud_entry', { src, dst })

export const previewCloudConflicts = (sources: string[], destDir: string) =>
  invoke<CloudConflictInfo[]>('preview_cloud_conflicts', { sources, dest_dir: destDir })

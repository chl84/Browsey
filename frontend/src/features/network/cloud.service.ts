import { normalizeError } from '@/shared/lib/error'
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

export type CloudRootSelection = {
  remote: CloudRemote
  rootPath: string
  isRemoteRoot: boolean
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

export type CloudWriteOptions = {
  overwrite?: boolean
}

const userCloudErrorMessage = (code: string | undefined, message: string) => {
  switch (code) {
    case 'binary_missing':
      return 'Cloud support requires rclone to be installed and available in PATH'
    case 'invalid_config':
      return 'The configured cloud remote is missing or invalid in rclone'
    case 'auth_required':
      return 'Cloud authentication is required or has expired. Reconnect the rclone remote and try again'
    case 'rate_limited':
      return 'Cloud provider rate limit reached. Wait a bit and try again'
    case 'timeout':
      return 'Cloud operation timed out. Check the network connection and try again'
    case 'network_error':
      return 'Cloud connection failed. Check the network and try again'
    case 'tls_certificate_error':
      return 'Cloud TLS certificate validation failed. Check the server certificate or trust settings and try again'
    case 'destination_exists':
      return 'A file or folder with the same name already exists'
    case 'permission_denied':
      return 'Cloud operation was denied (permissions or provider access)'
    case 'not_found':
      return 'Cloud file or folder was not found'
    case 'unsupported':
      return 'This cloud operation is not supported yet'
    case 'invalid_path':
      return 'Invalid cloud path'
    default: {
      const lower = message.toLowerCase()
      if (lower.includes('token') && (lower.includes('expired') || lower.includes('invalid'))) {
        return 'Cloud authentication may have expired. Reconnect the rclone remote and try again'
      }
      return message
    }
  }
}

const invokeCloud = async <T>(command: string, args?: Record<string, unknown>) => {
  try {
    return await invoke<T>(command, args)
  } catch (err) {
    const normalized = normalizeError(err)
    normalized.message = userCloudErrorMessage(normalized.code, normalized.message)
    throw normalized
  }
}

export const listCloudRemotes = () =>
  invokeCloud<CloudRemote[]>('list_cloud_remotes')

export const validateCloudRoot = (path: string) =>
  invokeCloud<CloudRootSelection>('validate_cloud_root', { path })

export const listCloudEntries = (path: string) =>
  invokeCloud<CloudEntry[]>('list_cloud_entries', { path })

export const statCloudEntry = (path: string) =>
  invokeCloud<CloudEntry | null>('stat_cloud_entry', { path })

export const normalizeCloudPath = (path: string) =>
  invokeCloud<string>('normalize_cloud_path', { path })

export const createCloudFolder = (path: string) =>
  invokeCloud<void>('create_cloud_folder', { path })

export const deleteCloudFile = (path: string) =>
  invokeCloud<void>('delete_cloud_file', { path })

export const deleteCloudDirRecursive = (path: string) =>
  invokeCloud<void>('delete_cloud_dir_recursive', { path })

export const deleteCloudDirEmpty = (path: string) =>
  invokeCloud<void>('delete_cloud_dir_empty', { path })

export const moveCloudEntry = (src: string, dst: string, options?: CloudWriteOptions) =>
  invokeCloud<void>('move_cloud_entry', { src, dst, overwrite: options?.overwrite ?? false })

export const renameCloudEntry = (src: string, dst: string, options?: CloudWriteOptions) =>
  invokeCloud<void>('rename_cloud_entry', { src, dst, overwrite: options?.overwrite ?? false })

export const copyCloudEntry = (src: string, dst: string, options?: CloudWriteOptions) =>
  invokeCloud<void>('copy_cloud_entry', { src, dst, overwrite: options?.overwrite ?? false })

export const previewCloudConflicts = (sources: string[], destDir: string) =>
  invokeCloud<CloudConflictInfo[]>('preview_cloud_conflicts', { sources, destDir })

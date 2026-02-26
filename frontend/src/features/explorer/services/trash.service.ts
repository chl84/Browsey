import { invoke } from '@/shared/lib/tauri'
import { deleteCloudDirRecursive, deleteCloudFile, statCloudEntry } from '@/features/network'

const isCloudPath = (path: string) => path.startsWith('rclone://')

export const deleteEntries = async (paths: string[], progressEvent?: string) => {
  const cloudCount = paths.filter(isCloudPath).length
  if (cloudCount === 0) {
    return invoke<void>('delete_entries', { paths, progressEvent })
  }
  if (cloudCount !== paths.length) {
    throw new Error('Mixed local/cloud delete is not supported yet')
  }

  for (const path of paths) {
    const entry = await statCloudEntry(path)
    if (!entry) {
      continue
    }
    if (entry.kind === 'dir') {
      await deleteCloudDirRecursive(path)
    } else {
      await deleteCloudFile(path)
    }
  }
}

export const moveToTrashMany = (paths: string[], progressEvent?: string) => {
  if (paths.some(isCloudPath)) {
    throw new Error('Cloud trash is not supported yet')
  }
  return invoke<void>('move_to_trash_many', { paths, progressEvent })
}

export const purgeTrashItems = (ids: string[]) =>
  invoke<void>('purge_trash_items', { ids })

export const restoreTrashItems = (ids: string[]) =>
  invoke<void>('restore_trash_items', { ids })

export const removeRecent = (paths: string[]) =>
  invoke<void>('remove_recent', { paths })

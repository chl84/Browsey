import { invoke } from '@/shared/lib/tauri'
import { deleteCloudDirRecursive, deleteCloudFile, statCloudEntry } from '@/features/network'
import { normalizeError } from '@/shared/lib/error'

const isCloudPath = (path: string) => path.startsWith('rclone://')
const isNotFoundError = (error: unknown) => normalizeError(error).code === 'not_found'

const cloudDeleteVerificationError = (path: string) =>
  new Error(
    `Cloud delete could not be verified for "${path}". Refresh and try again.`,
  )

const deleteCloudEntryWhenTypeUnknown = async (path: string, progressEvent?: string) => {
  let fileDeleteError: unknown
  try {
    await deleteCloudFile(path, progressEvent)
    return
  } catch (error) {
    if (!isNotFoundError(error)) {
      throw error
    }
    fileDeleteError = error
  }

  try {
    await deleteCloudDirRecursive(path, progressEvent)
    return
  } catch (error) {
    if (!isNotFoundError(error)) {
      throw error
    }
    if (isNotFoundError(fileDeleteError)) {
      throw cloudDeleteVerificationError(path)
    }
    throw error
  }
}

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
      await deleteCloudEntryWhenTypeUnknown(path, progressEvent)
      continue
    }
    if (entry.kind === 'dir') {
      await deleteCloudDirRecursive(path, progressEvent)
    } else {
      await deleteCloudFile(path, progressEvent)
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

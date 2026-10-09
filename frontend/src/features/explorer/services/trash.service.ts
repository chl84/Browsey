import { invoke } from '@/shared/lib/tauri'
import { deleteCloudDirRecursive, deleteCloudFile, statCloudEntry, trashCloudEntries } from '@/features/network'
import { normalizeError } from '@/shared/lib/error'
import type { ProgressPayload } from '../hooks/progress'

const isCloudPath = (path: string) => path.startsWith('rclone://')
const isNotFoundError = (error: unknown) => normalizeError(error).code === 'not_found'

// Read-only, lazy capability check for Wastebasket drops. Mutation still
// revalidates through the existing trash backends, with no permanent fallback.
export const canTrashPaths = (paths: string[]) => invoke<boolean>('can_trash_paths', { paths })

export const needsNetworkDeleteConfirmation = (error: unknown) =>
  normalizeError(error).code === 'network_confirmation_required'

const mutateNativeEntries = async (paths: string[], progressEvent: string | undefined, trash: boolean, confirmed: boolean) => {
  const networkPaths = await invoke<string[]>('network_delete_paths', { paths })
  // The mixed backend preflights remote trash before mutating any target and
  // owns one cancellation token/progress counter. Local targets keep undo.
  if (networkPaths.length) {
    return invoke<void>('network_delete_entries', { paths, trash, confirmed, progressEvent })
  }
  if (paths.length) {
    return invoke<void>(trash ? 'move_to_trash_many' : 'delete_entries', { paths, progressEvent })
  }
}

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

export const deleteEntries = async (paths: string[], progressEvent?: string, networkConfirmed = false, onProgress?: (payload: ProgressPayload) => void) => {
  const cloudCount = paths.filter(isCloudPath).length
  if (cloudCount === 0) {
    return mutateNativeEntries(paths, progressEvent, false, networkConfirmed)
  }
  if (cloudCount !== paths.length) {
    throw new Error('Mixed local/cloud delete is not supported yet')
  }

  const report = (items: number) => onProgress?.({ unit: 'items', items, total: paths.length, phase: 'Deleting cloud items…' })
  report(0)
  let completed = 0
  for (const path of paths) {
    const entry = await statCloudEntry(path)
    if (!entry) {
      await deleteCloudEntryWhenTypeUnknown(path, progressEvent)
      report(++completed)
      continue
    }
    if (entry.kind === 'dir') {
      await deleteCloudDirRecursive(path, progressEvent)
    } else {
      await deleteCloudFile(path, progressEvent)
    }
    report(++completed)
  }
}

export const moveToTrashMany = (paths: string[], progressEvent?: string, networkConfirmed = false) => {
  if (paths.some(isCloudPath)) {
    if (!paths.every(isCloudPath)) throw new Error('Mixed local/cloud trash is not supported')
    return trashCloudEntries(paths, progressEvent)
  }
  return mutateNativeEntries(paths, progressEvent, true, networkConfirmed)
}

export const purgeTrashItems = (ids: string[]) =>
  invoke<void>('purge_trash_items', { ids })

export const emptyTrash = () => invoke<void>('empty_trash')

export const restoreTrashItems = (ids: string[]) =>
  invoke<void>('restore_trash_items', { ids })

export const removeRecent = (paths: string[]) =>
  invoke<void>('remove_recent', { paths })

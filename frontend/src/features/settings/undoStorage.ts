import { writable, get } from 'svelte/store'
import { invoke } from '@/shared/lib/tauri'
import { getErrorMessage } from '@/shared/lib/error'
import { formatSize } from '@/shared/lib/formatSize'

export type UndoStorageSummary = {
  directory: string
  exists: boolean
  sessions: number
  markedSessions: number
  files: number
  logicalBytes: number
  allocatedBytes?: number | null
  backupCount?: number | null
  recoveredBackups?: number | null
  incomplete: boolean
}

export const inspectUndoStorage = () => invoke<UndoStorageSummary>('inspect_undo_storage')
export type DeleteBackupsResult = { deletedSessions: number; retainedSessions: number; errors: string[] }
export const deleteAllRecoveryBackups = () => invoke<DeleteBackupsResult>('delete_all_recovery_backups')

export const describeUndoStorageOverview = (summary: UndoStorageSummary) => {
  if (summary.incomplete) return `Partial scan: at least ${formatSize(summary.logicalBytes)} stored.`
  if (!summary.exists || summary.sessions === 0) return 'No backups found.'
  const { backupCount, recoveredBackups } = summary
  if (backupCount != null && recoveredBackups != null && Number.isInteger(backupCount)
    && Number.isInteger(recoveredBackups) && recoveredBackups > 0 && recoveredBackups <= backupCount) {
    const status = recoveredBackups === backupCount ? 'All backups recovered'
      : `${recoveredBackups} of ${backupCount} backups recovered`
    return `${formatSize(summary.logicalBytes)} stored · ${status}`
  }
  return `${formatSize(summary.logicalBytes)} stored in backups.`
}

export const describeUndoStorage = (summary: UndoStorageSummary) => {
  const prefix = summary.incomplete ? 'Incomplete scan — counted' : 'Last scan:'
  if (!summary.exists) return 'No undo storage directory exists yet.'
  const sessions = `${summary.sessions} ${summary.sessions === 1 ? 'session' : 'sessions'}`
  const marked = `${summary.markedSessions} ${summary.markedSessions === 1 ? 'session' : 'sessions'}`
  const allocation = summary.allocatedBytes == null
    ? '' : `; ${formatSize(summary.allocatedBytes)} allocated according to the filesystem`
  return `${prefix} ${formatSize(summary.logicalBytes)} of file contents in ${summary.files} files across ${sessions}${allocation}; ${marked} with recovery markers.`
}

export const createUndoStorageModel = (inspect = inspectUndoStorage, remove = deleteAllRecoveryBackups) => {
  const summary = writable<UndoStorageSummary | null>(null)
  const busy = writable(false)
  const error = writable('')
  const deleting = writable(false)
  const message = writable('')
  let disposed = false

  const refresh = async () => {
    if (disposed || get(busy)) return
    busy.set(true)
    error.set('')
    try {
      const result = await inspect()
      if (!disposed) summary.set(result)
    } catch (err) {
      if (!disposed) error.set(`Could not inspect backups: ${getErrorMessage(err)}`)
    } finally {
      if (!disposed) busy.set(false)
    }
  }

  const dispose = () => {
    disposed = true
    busy.set(false)
    summary.set(null)
    error.set('')
    message.set('')
  }

  const deleteAll = async () => {
    if (disposed || get(busy)) return false
    busy.set(true); deleting.set(true); error.set(''); message.set('')
    try {
      const result = await remove()
      if (!disposed) {
        message.set(result.retainedSessions
          ? `Undo history cleared. ${result.retainedSessions} backup ${result.retainedSessions === 1 ? 'session was' : 'sessions were'} kept because they are in use or could not be deleted.`
          : 'Backups and undo history deleted.')
        error.set(result.errors.join('\n'))
      }
      try {
        const next = await inspect()
        if (!disposed) summary.set(next)
      } catch (err) {
        if (!disposed) error.update(previous => [previous, `Could not refresh backup information: ${getErrorMessage(err)}`].filter(Boolean).join('\n'))
      }
      return true
    } catch (err) {
      if (!disposed) error.set(getErrorMessage(err))
      return false
    } finally {
      if (!disposed) { busy.set(false); deleting.set(false) }
    }
  }

  return { summary, busy, error, deleting, message, refresh, deleteAll, dispose }
}

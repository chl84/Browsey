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
  incomplete: boolean
}

export const inspectUndoStorage = () => invoke<UndoStorageSummary>('inspect_undo_storage')

export const describeUndoStorageOverview = (summary: UndoStorageSummary) => {
  if (summary.incomplete) return `Partial scan: at least ${formatSize(summary.logicalBytes)} stored.`
  if (!summary.exists || summary.sessions === 0) return 'No backups found.'
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

export const createUndoStorageModel = (inspect = inspectUndoStorage) => {
  const summary = writable<UndoStorageSummary | null>(null)
  const busy = writable(false)
  const error = writable('')
  let disposed = false

  const refresh = async () => {
    if (disposed || get(busy)) return
    busy.set(true)
    error.set('')
    try {
      const result = await inspect()
      if (!disposed) summary.set(result)
    } catch (err) {
      if (!disposed) error.set(getErrorMessage(err))
    } finally {
      if (!disposed) busy.set(false)
    }
  }

  const dispose = () => {
    disposed = true
    busy.set(false)
    summary.set(null)
    error.set('')
  }

  return { summary, busy, error, refresh, dispose }
}

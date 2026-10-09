import { get, writable } from 'svelte/store'
import { invoke } from '@/shared/lib/tauri'
import { getErrorMessage, normalizeError } from '@/shared/lib/error'
import { createActivity } from '@/features/explorer'

export type RecoveryBackup = {
  id: string
  version: string
  name: string
  kind: 'file' | 'dir'
  bytes: number | null
  modifiedAt: number | null
  blockedReason: string | null
}
export type RecoveryBackups = { entries: RecoveryBackup[]; incomplete: boolean }

export const createRecoveryBackupsModel = () => {
  const overview = writable<RecoveryBackups | null>(null)
  const loading = writable(false)
  const restoring = writable(false)
  const error = writable('')
  const notice = writable('')
  const restoredPath = writable('')
  const restoredName = writable('')
  const activityApi = createActivity({ onError: message => { if (!disposed) error.set(message) } })
  let disposed = false
  let cancelled = false
  let eventName = ''
  let restoreIssued = false

  const refresh = async () => {
    if (disposed || get(loading) || get(restoring)) return
    loading.set(true)
    error.set('')
    notice.set('')
    try {
      const result = await invoke<RecoveryBackups>('list_recovery_backups')
      if (!disposed) overview.set(result)
    } catch (err) { if (!disposed) error.set(getErrorMessage(err)) }
    finally { if (!disposed) loading.set(false) }
  }

  const cancel = async () => {
    cancelled = true
    if (restoreIssued && eventName) await activityApi.requestCancel(eventName)
  }

  const restore = async (backup: RecoveryBackup, destinationDir?: string) => {
    if (disposed || get(restoring) || get(loading) || backup.blockedReason || destinationDir === '') return
    restoring.set(true); error.set(''); notice.set(''); restoredPath.set(''); restoredName.set('')
    cancelled = false
    eventName = `recovery-${Date.now()}-${Math.random().toString(16).slice(2)}`
    let outcome: 'restored' | 'choose-destination' | undefined
    try {
      await activityApi.start('Recovering backup…', eventName, () => void cancel(), { completeOnReply: true })
      if (disposed || cancelled) return
      restoreIssued = true
      const path = await invoke<string>('restore_recovery_backup', {
        id: backup.id, version: backup.version, destinationDir: destinationDir ?? null, progressEvent: eventName,
      })
      if (!disposed) {
        overview.update(current => current ? { ...current,
          entries: current.entries.filter(entry => entry.id !== backup.id || entry.version !== backup.version),
        } : current)
        restoredPath.set(path); restoredName.set(backup.name); outcome = 'restored'
      }
    } catch (err) {
      if (!disposed) {
        const { code, details } = normalizeError(err)
        const occupied = !cancelled && destinationDir === undefined && code === 'recovery_destination_unavailable'
          && details !== null && typeof details === 'object' && 'reason' in details && details.reason === 'occupied'
        if (occupied) {
          error.set('')
          notice.set('Original location is occupied. Choose another folder.')
        } else {
          error.set(code === 'cancelled' || cancelled
            ? 'Recovery cancelled. The backup was kept. Any incomplete copy remains in the destination folder.'
            : getErrorMessage(err))
        }
        if (!cancelled && destinationDir === undefined && code === 'recovery_destination_unavailable') outcome = 'choose-destination'
      }
    } finally {
      restoreIssued = false
      activityApi.clearNow()
      await activityApi.cleanup()
      eventName = ''
      if (!disposed) restoring.set(false)
    }
    return disposed ? undefined : outcome
  }

  const openFolder = async (directory: string) => {
    try { await invoke('open_entry', { path: directory }) }
    catch (err) { if (!disposed) error.set(getErrorMessage(err)) }
  }

  const dispose = () => {
    disposed = true
    void cancel()
    if (!get(restoring)) { activityApi.clearNow(); void activityApi.cleanup() }
  }
  return { overview, loading, restoring, error, notice, restoredPath, restoredName, activity: activityApi.activity,
    refresh, restore, cancel, openFolder, dispose }
}

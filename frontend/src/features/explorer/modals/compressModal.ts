import { invoke } from '@/shared/lib/tauri'
import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  cleanup: (preserveTimer?: boolean) => Promise<void>
  clearNow: () => void
  requestCancel: (eventName: string) => Promise<void>
}

type Deps = {
  activityApi: ActivityApi
  reloadCurrent: () => Promise<void>
  showToast: (msg: string) => void
}

export type CompressState = {
  open: boolean
  targets: Entry[]
  error: string
}

export const createCompressModal = (deps: Deps) => {
  const { activityApi, reloadCurrent, showToast } = deps
  const state = writable<CompressState>({ open: false, targets: [], error: '' })
  let busy = false
  let activeEvent: string | null = null

  const open = (entries: Entry[], defaultBase: string) => {
    state.set({ open: true, targets: entries, error: '' })
    const base = defaultBase && defaultBase.trim().length > 0 ? defaultBase.trim() : 'Archive'
    const defaultName =
      entries.length === 1
        ? entries[0].name.toLowerCase().endsWith('.zip')
          ? entries[0].name.slice(0, -4)
          : entries[0].name
        : base
    return defaultName
  }

  const close = () => state.set({ open: false, targets: [], error: '' })
  const cancelOrClose = () => {
    if (!busy) {
      close()
    } else if (activeEvent) {
      void activityApi.requestCancel(activeEvent).catch((error) => {
        state.update((s) => ({ ...s, error: getErrorMessage(error) }))
      })
    }
  }

  const confirm = async (name: string, level: number) => {
    const current = get(state)
    if (!current.open || current.targets.length === 0 || busy) {
      return false
    }
    busy = true
    state.update((s) => ({ ...s, error: '' }))
    const lvl = Math.min(Math.max(Math.round(level), 0), 9)
    const paths = current.targets.map((e) => e.path)
    const progressEvent = `compress-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    activeEvent = progressEvent
    try {
      await activityApi.start('Compressing…', progressEvent, () => activityApi.requestCancel(progressEvent))
      const base = (name || '').trim().replace(/\.zip$/i, '')
      const finalName = base.length > 0 ? `${base}.zip` : 'Archive.zip'
      const dest = await invoke<string>('compress_entries', {
        paths,
        name: finalName,
        level: lvl,
        progressEvent,
      })
      if (reloadCurrent) {
        await reloadCurrent()
      }
      close()
      showToast(`Created ${dest}`)
      return true
    } catch (err) {
      const msg = getErrorMessage(err)
      if (msg.toLowerCase().includes('cancelled')) {
        state.update((s) => ({ ...s, error: '' }))
        close()
        showToast('Compression cancelled')
      } else {
        state.update((s) => ({ ...s, error: msg }))
      }
      return false
    } finally {
      busy = false
      activeEvent = null
      activityApi.clearNow()
      await activityApi.cleanup()
    }
  }

  return {
    state,
    open,
    close: cancelOrClose,
    confirm,
  }
}

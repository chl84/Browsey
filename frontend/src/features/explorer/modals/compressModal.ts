import { invoke } from '@tauri-apps/api/core'
import { writable, get } from 'svelte/store'
import type { Entry } from '../types'

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  cleanup: (preserveTimer?: boolean) => Promise<void>
  clearNow: () => void
  requestCancel: (eventName: string) => Promise<void>
}

type Deps = {
  activityApi: ActivityApi
  getCurrentPath: () => string | null
  loadPath: (path: string) => Promise<void>
  showToast: (msg: string) => void
}

export type CompressState = {
  open: boolean
  targets: Entry[]
  error: string
}

export const createCompressModal = (deps: Deps) => {
  const { activityApi, getCurrentPath, loadPath, showToast } = deps
  const state = writable<CompressState>({ open: false, targets: [], error: '' })
  let busy = false

  const open = (entries: Entry[]) => {
    state.set({ open: true, targets: entries, error: '' })
    const defaultName =
      entries.length === 1
        ? entries[0].name.toLowerCase().endsWith('.zip')
          ? entries[0].name
          : `${entries[0].name}.zip`
        : 'Archive.zip'
    return defaultName
  }

  const close = () => state.set({ open: false, targets: [], error: '' })

  const confirm = async (name: string, level: number) => {
    const current = get(state)
    if (!current.open || current.targets.length === 0 || busy) {
      close()
      return false
    }
    busy = true
    const lvl = Math.min(Math.max(Math.round(level), 0), 9)
    const base = getCurrentPath()
    if (!base) {
      showToast('No current path')
      busy = false
      return false
    }
    const paths = current.targets.map((e) => e.path)
    const progressEvent = `compress-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi.start('Compressing…', progressEvent, () => activityApi.requestCancel(progressEvent))
      const dest = await invoke<string>('compress_entries', {
        paths,
        name,
        level: lvl,
        progressEvent,
      })
      await loadPath(base)
      close()
      showToast(`Created ${dest}`)
      return true
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
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
      activityApi.clearNow()
      await activityApi.cleanup()
    }
  }

  return {
    state,
    open,
    close,
    confirm,
  }
}

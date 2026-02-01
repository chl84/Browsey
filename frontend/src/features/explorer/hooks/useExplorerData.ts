import { onMount } from 'svelte'
import { get } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Entry } from '../types'
import { createExplorerState } from '../state'

type Options = {
  onEntriesChanged?: () => void
  onCurrentChange?: (path: string) => void
  initialPath?: string
  partitionsPollMs?: number
}

export const useExplorerData = (options: Options = {}) => {
  const explorer = createExplorerState({
    onEntriesChanged: options.onEntriesChanged,
    onCurrentChange: options.onCurrentChange,
  })

  const {
    load,
    loadSavedWidths,
    loadBookmarks,
    loadPartitions,
    loadShowHiddenPref,
    loadHiddenFilesLastPref,
    loadFoldersFirstPref,
    loadStartDirPref,
    loadConfirmDeletePref,
    loadSortPref,
    loadDensityPref,
    entries,
    current,
    startDirPref,
    sortFieldPref,
    sortDirectionPref,
  } = explorer

  let partitionsPoll: ReturnType<typeof setInterval> | null = null
  let unlistenDirChanged: UnlistenFn | null = null
  let unlistenEntryMeta: UnlistenFn | null = null
  let unlistenEntryMetaBatch: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null

  const setup = async () => {
    void loadSavedWidths()
    void loadBookmarks()
    void loadPartitions()
    await Promise.all([
      loadShowHiddenPref(),
      loadHiddenFilesLastPref(),
      loadFoldersFirstPref(),
      loadStartDirPref(),
      loadConfirmDeletePref(),
      loadSortPref(),
      loadDensityPref(),
    ])

    const pollMs = options.partitionsPollMs ?? 2000
    if (pollMs > 0) {
      partitionsPoll = setInterval(() => {
        void loadPartitions()
      }, pollMs)
    }

    const initial = options.initialPath ?? (get(startDirPref) ?? undefined)
    await load(initial)

    unlistenDirChanged = await listen<string>('dir-changed', (event) => {
      const curr = get(current)
      const payload = event.payload
      if (!curr || payload === curr) {
        if (refreshTimer) {
          clearTimeout(refreshTimer)
        }
        refreshTimer = setTimeout(() => {
          const latest = get(current)
          if (!latest || latest !== payload) return
          void load(latest, { recordHistory: false, silent: true })
        }, 300)
      }
    })

    unlistenEntryMeta = await listen<Entry>('entry-meta', (event) => {
      const update = event.payload
      entries.update((list) => {
        const idx = list.findIndex((e) => e.path === update.path)
        if (idx === -1) return list
        const next = [...list]
        next[idx] = { ...next[idx], ...update }
        return next
      })
    })

    unlistenEntryMetaBatch = await listen<Entry[]>('entry-meta-batch', (event) => {
      const updates = event.payload
      if (!Array.isArray(updates) || updates.length === 0) return
      const map = new Map(updates.map((u) => [u.path, u]))
      entries.update((list) => list.map((item) => (map.has(item.path) ? { ...item, ...map.get(item.path)! } : item)))
    })
  }

  const cleanup = () => {
    if (refreshTimer) {
      clearTimeout(refreshTimer)
      refreshTimer = null
    }
    if (partitionsPoll) {
      clearInterval(partitionsPoll)
      partitionsPoll = null
    }
    if (unlistenDirChanged) {
      unlistenDirChanged()
      unlistenDirChanged = null
    }
    if (unlistenEntryMeta) {
      unlistenEntryMeta()
      unlistenEntryMeta = null
    }
    if (unlistenEntryMetaBatch) {
      unlistenEntryMetaBatch()
      unlistenEntryMetaBatch = null
    }
  }

  onMount(() => {
    void setup()
    return cleanup
  })

  return {
    ...explorer,
    cleanup,
  }
}

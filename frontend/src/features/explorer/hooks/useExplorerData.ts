import { onMount } from 'svelte'
import { get } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Entry } from '../types'
import { createExplorerState } from '../state'

const isGvfsPath = (path: string | null | undefined) =>
  !!path && path.includes('/run/user/') && path.includes('/gvfs/')

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
    loadArchiveNamePref,
    loadArchiveLevelPref,
    loadOpenDestAfterExtractPref,
    loadVideoThumbsPref,
    loadFfmpegPathPref,
    loadThumbCachePref,
    entries,
    current,
    startDirPref,
    sortFieldPref,
    sortDirectionPref,
    openDestAfterExtract,
  } = explorer

  let partitionsPoll: ReturnType<typeof setInterval> | null = null
  let unlistenDirChanged: UnlistenFn | null = null
  let unlistenEntryMeta: UnlistenFn | null = null
  let unlistenEntryMetaBatch: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null
  let gvfsRefresh: ReturnType<typeof setInterval> | null = null
  let gvfsInFlightPath: string | null = null
  let gvfsRefreshGen = 0
  let unsubscribeCurrent: (() => void) | null = null

  const cancelGvfsRefresh = () => {
    gvfsRefreshGen += 1
    gvfsInFlightPath = null
  }

  const refreshGvfsPath = (path: string | null | undefined) => {
    if (!path || !isGvfsPath(path)) return
    // Debounce: skip if same path already refreshing
    if (gvfsInFlightPath === path) return
    // Early bail: path already changed
    if (get(current) !== path) return
    const gen = gvfsRefreshGen
    gvfsInFlightPath = path
    void (async () => {
      try {
        // If a user navigation happened after we scheduled this, skip
        if (gen !== gvfsRefreshGen) return
        await load(path, { recordHistory: false, silent: true })
      } finally {
        // Clear only if we still point at the same path
        if (gen === gvfsRefreshGen && gvfsInFlightPath === path) {
          gvfsInFlightPath = null
        }
      }
    })()
  }

  const ensureGvfsRefresh = (path: string | null | undefined) => {
    const shouldPoll = isGvfsPath(path)
    if (shouldPoll) {
      if (!gvfsRefresh) {
        gvfsRefresh = setInterval(() => {
          const latest = get(current)
          if (!latest || !isGvfsPath(latest)) return
          refreshGvfsPath(latest)
        }, 5000)
      }
    } else if (gvfsRefresh) {
      clearInterval(gvfsRefresh)
      gvfsRefresh = null
      gvfsInFlightPath = null
    }
  }

  const loadWithCancel = async (
    path?: Parameters<typeof load>[0],
    opts?: Parameters<typeof load>[1]
  ) => {
    cancelGvfsRefresh()
    return load(path, opts)
  }

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
      loadArchiveNamePref(),
      loadArchiveLevelPref(),
      loadOpenDestAfterExtractPref(),
      loadVideoThumbsPref(),
      loadFfmpegPathPref(),
      loadThumbCachePref(),
    ])

    const pollMs = options.partitionsPollMs ?? 8000
    if (pollMs > 0) {
      partitionsPoll = setInterval(() => {
        void loadPartitions()
      }, pollMs)
    }

    const initial = options.initialPath ?? (get(startDirPref) ?? undefined)
    await loadWithCancel(initial)
    ensureGvfsRefresh(get(current))
    unsubscribeCurrent = current.subscribe((p) => ensureGvfsRefresh(p))

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
    if (gvfsRefresh) {
      clearInterval(gvfsRefresh)
      gvfsRefresh = null
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
    if (unsubscribeCurrent) {
      unsubscribeCurrent()
      unsubscribeCurrent = null
    }
  }

  onMount(() => {
    void setup()
    return cleanup
  })

  return {
    ...explorer,
    load: loadWithCancel,
    cleanup,
  }
}

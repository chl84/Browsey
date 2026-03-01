import { onMount } from 'svelte'
import { get } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Entry } from '../model/types'
import { createExplorerState } from '../state'

const isGvfsPath = (path: string | null | undefined) =>
  !!path && path.includes('/run/user/') && path.includes('/gvfs/')
const isCloudPath = (path: string | null | undefined) => !!path && path.startsWith('rclone://')
const CLOUD_DIR_REFRESHED_EVENT = 'cloud-dir-refreshed'

type CloudDirRefreshedEvent = {
  path: string
  entryCount?: number
}

type Options = {
  onEntriesChanged?: () => void
  onCurrentChange?: (path: string) => void
  onOpenEntry?: (entry: Entry) => void | Promise<void>
  initialPath?: string
  partitionsPollMs?: number
}

export const useExplorerData = (options: Options = {}) => {
  const explorer = createExplorerState({
    onEntriesChanged: options.onEntriesChanged,
    onCurrentChange: options.onCurrentChange,
    onOpenEntry: options.onOpenEntry,
  })

  const {
    load,
    mountsPollMs,
    loadSavedWidths,
    loadBookmarks,
    loadPartitions,
    loadMountsPollPref,
    loadShowHiddenPref,
    loadHiddenFilesLastPref,
    loadHighContrastPref,
    loadScrollbarWidthPref,
    loadFoldersFirstPref,
    loadStartDirPref,
    loadConfirmDeletePref,
    loadSortPref,
    loadDensityPref,
    loadArchiveNamePref,
    loadArchiveLevelPref,
    loadOpenDestAfterExtractPref,
    loadVideoThumbsPref,
    loadHardwareAccelerationPref,
    loadFfmpegPathPref,
    loadThumbCachePref,
    loadDoubleClickMsPref,
    entries,
    current,
    highContrast,
    scrollbarWidth,
    startDirPref,
    invalidateFacetCache,
  } = explorer

  let partitionsPoll: ReturnType<typeof setInterval> | null = null
  let unlistenDirChanged: UnlistenFn | null = null
  let unlistenEntryMeta: UnlistenFn | null = null
  let unlistenEntryMetaBatch: UnlistenFn | null = null
  let unlistenCloudDirRefreshed: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null
  let gvfsRefresh: ReturnType<typeof setInterval> | null = null
  let gvfsInFlightPath: string | null = null
  let cloudRefreshInFlightPath: string | null = null
  let unsubscribeCurrent: (() => void) | null = null
  let userNavActive = false
  let userNavGen = 0
  let unsubscribeMountsPoll: (() => void) | null = null
  let unsubscribeHighContrast: (() => void) | null = null
  let unsubscribeScrollbarWidth: (() => void) | null = null
  let metaQueue = new Map<string, Partial<Entry>>()
  let metaTimer: ReturnType<typeof setTimeout> | null = null
  let disposed = false

  const applyHighContrastRootState = (enabled: boolean) => {
    if (typeof document === 'undefined') return
    document.documentElement.dataset.highContrast = enabled ? 'true' : 'false'
  }

  const applyScrollbarWidthRootState = (width: number) => {
    if (typeof document === 'undefined') return
    document.documentElement.style.setProperty('--scrollbar-size', `${width}px`)
  }

  const refreshGvfsPath = (path: string | null | undefined) => {
    if (!path || !isGvfsPath(path)) return
    if (userNavActive) return
    // Debounce: skip if same path already refreshing
    if (gvfsInFlightPath === path) return
    // Early bail: path already changed
    if (get(current) !== path) return
    gvfsInFlightPath = path
    void (async () => {
      try {
        await load(path, { recordHistory: false, silent: true })
      } finally {
        // Clear only if we still point at the same path
        if (gvfsInFlightPath === path) {
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

  const refreshCloudPath = (path: string | null | undefined) => {
    if (!path || !isCloudPath(path)) return
    if (userNavActive) return
    if (cloudRefreshInFlightPath === path) return
    if (get(current) !== path) return
    cloudRefreshInFlightPath = path
    void (async () => {
      try {
        await load(path, { recordHistory: false, silent: true })
      } finally {
        if (cloudRefreshInFlightPath === path) {
          cloudRefreshInFlightPath = null
        }
      }
    })()
  }

  const setup = async () => {
    if (disposed) return
    void loadSavedWidths()
    void loadBookmarks()
    void loadPartitions()
    await loadMountsPollPref()
    if (disposed) return
    await Promise.all([
      loadShowHiddenPref(),
      loadHiddenFilesLastPref(),
      loadHighContrastPref(),
      loadScrollbarWidthPref(),
      loadFoldersFirstPref(),
      loadStartDirPref(),
      loadConfirmDeletePref(),
      loadSortPref(),
      loadDensityPref(),
      loadArchiveNamePref(),
      loadArchiveLevelPref(),
      loadOpenDestAfterExtractPref(),
      loadVideoThumbsPref(),
      loadHardwareAccelerationPref(),
      loadFfmpegPathPref(),
      loadThumbCachePref(),
      loadDoubleClickMsPref(),
    ])
    if (disposed) return

    const setPartitionsPollMs = (ms: number) => {
      if (disposed) return
      if (partitionsPoll) {
        clearInterval(partitionsPoll)
        partitionsPoll = null
      }
      if (ms > 0) {
        partitionsPoll = setInterval(() => {
          void loadPartitions()
        }, ms)
      }
    }

    setPartitionsPollMs(options.partitionsPollMs ?? get(mountsPollMs))
    if (disposed) return

    unsubscribeMountsPoll = mountsPollMs.subscribe((v) => {
      setPartitionsPollMs(v)
    })
    if (disposed) {
      unsubscribeMountsPoll()
      unsubscribeMountsPoll = null
      return
    }

    applyHighContrastRootState(get(highContrast))
    unsubscribeHighContrast = highContrast.subscribe((enabled) => {
      applyHighContrastRootState(enabled)
    })
    if (disposed) {
      unsubscribeHighContrast()
      unsubscribeHighContrast = null
      return
    }

    applyScrollbarWidthRootState(get(scrollbarWidth))
    unsubscribeScrollbarWidth = scrollbarWidth.subscribe((width) => {
      applyScrollbarWidthRootState(width)
    })
    if (disposed) {
      unsubscribeScrollbarWidth()
      unsubscribeScrollbarWidth = null
      return
    }

    const initial = options.initialPath ?? (get(startDirPref) ?? undefined)
    await load(initial)
    if (disposed) return
    ensureGvfsRefresh(get(current))
    unsubscribeCurrent = current.subscribe((p) => ensureGvfsRefresh(p))
    if (disposed) {
      unsubscribeCurrent()
      unsubscribeCurrent = null
      return
    }

    const unlistenDir = await listen<string>('dir-changed', (event) => {
      if (disposed) return
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
    if (disposed) {
      unlistenDir()
      return
    }
    unlistenDirChanged = unlistenDir

    const unlistenCloudRefresh = await listen<CloudDirRefreshedEvent>(
      CLOUD_DIR_REFRESHED_EVENT,
      (event) => {
        if (disposed) return
        refreshCloudPath(event.payload?.path)
      },
    )
    if (disposed) {
      await unlistenCloudRefresh()
      return
    }
    unlistenCloudDirRefreshed = unlistenCloudRefresh

    const unlistenMeta = await listen<Entry>('entry-meta', (event) => {
      if (disposed) return
      enqueueMetaUpdate(event.payload)
    })
    if (disposed) {
      unlistenMeta()
      return
    }
    unlistenEntryMeta = unlistenMeta

    const unlistenMetaBatch = await listen<Entry[]>('entry-meta-batch', (event) => {
      if (disposed) return
      const updates = event.payload
      if (!Array.isArray(updates) || updates.length === 0) return
      updates.forEach(enqueueMetaUpdate)
    })
    if (disposed) {
      unlistenMetaBatch()
      return
    }
    unlistenEntryMetaBatch = unlistenMetaBatch
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
    if (unsubscribeMountsPoll) {
      unsubscribeMountsPoll()
      unsubscribeMountsPoll = null
    }
    if (unsubscribeHighContrast) {
      unsubscribeHighContrast()
      unsubscribeHighContrast = null
    }
    if (unsubscribeScrollbarWidth) {
      unsubscribeScrollbarWidth()
      unsubscribeScrollbarWidth = null
    }
    if (gvfsRefresh) {
      clearInterval(gvfsRefresh)
      gvfsRefresh = null
    }
    gvfsInFlightPath = null
    cloudRefreshInFlightPath = null
    if (metaTimer) {
      clearTimeout(metaTimer)
      metaTimer = null
    }
    metaQueue.clear()
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
    if (unlistenCloudDirRefreshed) {
      unlistenCloudDirRefreshed()
      unlistenCloudDirRefreshed = null
    }
    if (unsubscribeCurrent) {
      unsubscribeCurrent()
      unsubscribeCurrent = null
    }
  }

  onMount(() => {
    void setup()
    return () => {
      disposed = true
      cleanup()
    }
  })

  const flushMetaQueue = () => {
    if (metaQueue.size === 0) return
    const pending = metaQueue
    metaQueue = new Map()
    invalidateFacetCache()
    entries.update((list) =>
      list.map((item) => {
        const upd = pending.get(item.path)
        return upd ? { ...item, ...upd } : item
      }),
    )
  }

  const enqueueMetaUpdate = (update: Entry) => {
    metaQueue.set(update.path, { ...metaQueue.get(update.path), ...update })
    if (metaTimer) return
    metaTimer = setTimeout(() => {
      metaTimer = null
      flushMetaQueue()
    }, 50)
  }

  const loadUserNav = async (
    path?: Parameters<typeof load>[0],
    opts?: Parameters<typeof load>[1]
  ) => {
    userNavActive = true
    const gen = ++userNavGen
    try {
      return await load(path, opts)
    } finally {
      if (userNavGen === gen) {
        userNavActive = false
      }
    }
  }

  return {
    ...explorer,
    load: loadUserNav,
    cleanup,
  }
}

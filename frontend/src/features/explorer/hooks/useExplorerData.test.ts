import { writable } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const { listenMock, eventHandlers, createExplorerStateMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  eventHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  createExplorerStateMock: vi.fn(),
}))

vi.mock('svelte', async () => {
  const actual = await vi.importActual<typeof import('svelte')>('svelte')
  return {
    ...actual,
    onMount: (fn: () => void | (() => void)) => fn(),
  }
})

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('../state', () => ({
  createExplorerState: createExplorerStateMock,
}))

import { useExplorerData } from './useExplorerData'

const asyncNoop = vi.fn(async () => {})

describe('useExplorerData cloud refresh event', () => {
  beforeEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
    eventHandlers.clear()
    listenMock.mockImplementation(async (eventName: string, handler: (event: { payload: unknown }) => void) => {
      eventHandlers.set(eventName, handler)
      return async () => {
        eventHandlers.delete(eventName)
      }
    })
  })

  const installExplorerStateMock = (currentPath: string) => {
    const current = writable(currentPath)
    const entries = writable([])
    const mountsPollMs = writable(0)
    const highContrast = writable(false)
    const scrollbarWidth = writable(10)
    const startDirPref = writable<string | null>(null)
    const loadMock = vi.fn(async (path?: string) => {
      if (path) {
        current.set(path)
      }
    })
    const loadDetailedMock = vi.fn(async (path?: string) => {
      if (path) {
        current.set(path)
      }
      return { ok: true, code: undefined as string | undefined, message: undefined as string | undefined }
    })
    const loadPartitionsMock = vi.fn(async () => {})

    createExplorerStateMock.mockReturnValue({
      load: loadMock,
      loadDetailed: loadDetailedMock,
      mountsPollMs,
      loadSavedWidths: asyncNoop,
      loadBookmarks: asyncNoop,
      loadPartitions: loadPartitionsMock,
      loadMountsPollPref: asyncNoop,
      loadShowHiddenPref: asyncNoop,
      loadHiddenFilesLastPref: asyncNoop,
      loadHighContrastPref: asyncNoop,
      loadScrollbarWidthPref: asyncNoop,
      loadRclonePathPref: asyncNoop,
      loadFoldersFirstPref: asyncNoop,
      loadStartDirPref: asyncNoop,
      loadConfirmDeletePref: asyncNoop,
      loadSortPref: asyncNoop,
      loadDensityPref: asyncNoop,
      loadArchiveNamePref: asyncNoop,
      loadArchiveLevelPref: asyncNoop,
      loadOpenDestAfterExtractPref: asyncNoop,
      loadVideoThumbsPref: asyncNoop,
      loadCloudThumbsPref: asyncNoop,
      loadCloudEnabledPref: asyncNoop,
      loadHardwareAccelerationPref: asyncNoop,
      loadFfmpegPathPref: asyncNoop,
      loadThumbCachePref: asyncNoop,
      loadDoubleClickMsPref: asyncNoop,
      loadLogLevelPref: asyncNoop,
      entries,
      current,
      highContrast,
      scrollbarWidth,
      startDirPref,
      invalidateFacetCache: vi.fn(),
    })

    return {
      loadMock,
      loadDetailedMock,
      current,
      mountsPollMs,
      highContrast,
      scrollbarWidth,
      loadPartitionsMock,
    }
  }

  it('reloads the active cloud directory when background refresh completes', async () => {
    const { loadMock } = installExplorerStateMock('rclone://work/docs')

    useExplorerData()
    await vi.waitFor(() => {
      expect(eventHandlers.get('cloud-dir-refreshed')).toBeTypeOf('function')
    })
    loadMock.mockClear()

    const handler = eventHandlers.get('cloud-dir-refreshed')
    handler?.({ payload: { path: 'rclone://work/docs', entryCount: 3 } })
    await vi.waitFor(() => {
      expect(loadMock).toHaveBeenCalledWith('rclone://work/docs', {
        recordHistory: false,
        silent: true,
      })
    })

    expect(loadMock).toHaveBeenCalledTimes(1)
  })

  it('ignores background refresh events for other directories', async () => {
    const { loadMock } = installExplorerStateMock('rclone://work/docs')

    useExplorerData()
    await vi.waitFor(() => {
      expect(eventHandlers.get('cloud-dir-refreshed')).toBeTypeOf('function')
    })
    loadMock.mockClear()

    const handler = eventHandlers.get('cloud-dir-refreshed')
    handler?.({ payload: { path: 'rclone://work/other' } })
    await Promise.resolve()

    expect(loadMock).not.toHaveBeenCalled()
  })

  it('applies the high-contrast root hook from explorer state', async () => {
    const { highContrast } = installExplorerStateMock('~')

    delete document.documentElement.dataset.highContrast
    useExplorerData()

    await vi.waitFor(() => {
      expect(document.documentElement.dataset.highContrast).toBe('false')
    })

    highContrast.set(true)
    await vi.waitFor(() => {
      expect(document.documentElement.dataset.highContrast).toBe('true')
    })
  })

  it('applies the scrollbar width root hook from explorer state', async () => {
    const { scrollbarWidth } = installExplorerStateMock('~')

    document.documentElement.style.removeProperty('--scrollbar-size')
    useExplorerData()

    await vi.waitFor(() => {
      expect(document.documentElement.style.getPropertyValue('--scrollbar-size')).toBe('10px')
    })

    scrollbarWidth.set(16)
    await vi.waitFor(() => {
      expect(document.documentElement.style.getPropertyValue('--scrollbar-size')).toBe('16px')
    })
  })

  it('applies mount refresh interval changes without restart', async () => {
    vi.useFakeTimers()
    const { mountsPollMs, loadPartitionsMock } = installExplorerStateMock('~')
    mountsPollMs.set(40)

    useExplorerData()
    await vi.waitFor(() => {
      expect(loadPartitionsMock).toHaveBeenCalledTimes(1)
    })

    await vi.advanceTimersByTimeAsync(40)
    expect(loadPartitionsMock).toHaveBeenCalledTimes(2)

    mountsPollMs.set(120)
    await Promise.resolve()
    await vi.advanceTimersByTimeAsync(80)
    expect(loadPartitionsMock).toHaveBeenCalledTimes(2)

    await vi.advanceTimersByTimeAsync(40)
    expect(loadPartitionsMock).toHaveBeenCalledTimes(3)
  })

  it('starts cancellable activity for interactive cloud directory loads and hides it on success', async () => {
    const { loadDetailedMock } = installExplorerStateMock('~')
    const activityApi = {
      start: vi.fn(async () => {}),
      requestCancel: vi.fn(async () => {}),
      clearNow: vi.fn(),
      cleanup: vi.fn(async () => {}),
      hideSoon: vi.fn(),
      hasHideTimer: vi.fn(() => false),
      activity: writable(null),
    }

    const explorer = useExplorerData({ activityApi })
    await explorer.load('rclone://work/docs')

    expect(activityApi.start).toHaveBeenCalledTimes(1)
    expect(activityApi.start).toHaveBeenCalledWith(
      'Loading cloud folder…',
      expect.stringMatching(/^cloud-list-/),
      expect.any(Function),
    )
    expect(loadDetailedMock).toHaveBeenCalledWith(
      'rclone://work/docs',
      expect.objectContaining({
        progressEvent: expect.stringMatching(/^cloud-list-/),
        showLoadingIndicator: false,
      }),
    )
    expect(activityApi.hideSoon).toHaveBeenCalledTimes(1)
    expect(activityApi.clearNow).not.toHaveBeenCalled()
  })

  it('clears activity without surfacing a generic error when cloud directory load is cancelled', async () => {
    const { loadDetailedMock } = installExplorerStateMock('~')
    loadDetailedMock.mockResolvedValueOnce({
      ok: false,
      code: 'cancelled',
      message: 'Cloud folder loading cancelled',
    })
    const activityApi = {
      start: vi.fn(async () => {}),
      requestCancel: vi.fn(async () => {}),
      clearNow: vi.fn(),
      cleanup: vi.fn(async () => {}),
      hideSoon: vi.fn(),
      hasHideTimer: vi.fn(() => false),
      activity: writable(null),
    }

    const explorer = useExplorerData({ activityApi })
    await explorer.load('rclone://work/docs')

    expect(activityApi.clearNow).toHaveBeenCalledTimes(1)
    expect(activityApi.cleanup).toHaveBeenCalledTimes(1)
    expect(activityApi.hideSoon).not.toHaveBeenCalled()
  })
})

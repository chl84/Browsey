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

    createExplorerStateMock.mockReturnValue({
      load: loadMock,
      mountsPollMs,
      loadSavedWidths: asyncNoop,
      loadBookmarks: asyncNoop,
      loadPartitions: asyncNoop,
      loadMountsPollPref: asyncNoop,
      loadShowHiddenPref: asyncNoop,
      loadHiddenFilesLastPref: asyncNoop,
      loadHighContrastPref: asyncNoop,
      loadScrollbarWidthPref: asyncNoop,
      loadFoldersFirstPref: asyncNoop,
      loadStartDirPref: asyncNoop,
      loadConfirmDeletePref: asyncNoop,
      loadSortPref: asyncNoop,
      loadDensityPref: asyncNoop,
      loadArchiveNamePref: asyncNoop,
      loadArchiveLevelPref: asyncNoop,
      loadOpenDestAfterExtractPref: asyncNoop,
      loadVideoThumbsPref: asyncNoop,
      loadHardwareAccelerationPref: asyncNoop,
      loadFfmpegPathPref: asyncNoop,
      loadThumbCachePref: asyncNoop,
      loadDoubleClickMsPref: asyncNoop,
      entries,
      current,
      highContrast,
      scrollbarWidth,
      startDirPref,
      invalidateFacetCache: vi.fn(),
    })

    return { loadMock, current, highContrast, scrollbarWidth }
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
})

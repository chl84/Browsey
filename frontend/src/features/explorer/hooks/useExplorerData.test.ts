import { writable } from 'svelte/store'
import type { Entry } from '../model/types'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const { listenMock, eventHandlers, createExplorerStateMock, getStartupPathMock, cleanups } = vi.hoisted(() => ({
  cleanups: [] as Array<() => void>,
  listenMock: vi.fn(),
  eventHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  createExplorerStateMock: vi.fn(),
  getStartupPathMock: vi.fn(),
}))

vi.mock('svelte', async () => {
  const actual = await vi.importActual<typeof import('svelte')>('svelte')
  return {
    ...actual,
    onMount: (fn: () => void | (() => void)) => {
      const cleanup = fn()
      if (cleanup) cleanups.push(cleanup)
    },
  }
})

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('../state', () => ({
  createExplorerState: createExplorerStateMock,
}))

vi.mock('../services/listing.service', () => ({
  getStartupPath: getStartupPathMock,
}))

import { useExplorerData } from './useExplorerData'

const asyncNoop = vi.fn(async () => {})

describe('useExplorerData cloud refresh event', () => {
  afterEach(() => {
    cleanups.splice(0).forEach((cleanup) => cleanup())
    vi.useRealTimers()
  })
  beforeEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
    getStartupPathMock.mockResolvedValue(null)
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
    const entries = writable<Entry[]>([])
    const searchMode = writable(false)
    const loading = writable(false)
    const applyEntryMetadata = vi.fn()
    const mountsPollMs = writable(0)
    const highContrast = writable(false)
    const scrollbarWidth = writable(10)
    const startDirPref = writable<string | null>(null)
    const error = writable('')
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
      searchMode,
      loading,
      applyEntryMetadata,
      current,
      highContrast,
      scrollbarWidth,
      startDirPref,
      error,
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
      startDirPref,
      error,
      entries,
      searchMode,
      loading,
      applyEntryMetadata,
    }
  }

  it('opens the requested launch folder instead of the saved start folder', async () => {
    const { loadMock, startDirPref } = installExplorerStateMock('')
    startDirPref.set('/saved/home')
    getStartupPathMock.mockResolvedValue('/project/æ folder #100%')

    useExplorerData()
    await vi.waitFor(() => expect(loadMock).toHaveBeenCalledWith('/project/æ folder #100%'))
    expect(loadMock).toHaveBeenCalledTimes(1)
  })

  it('preserves the saved start folder when launched without a folder argument', async () => {
    const { loadMock, startDirPref } = installExplorerStateMock('')
    startDirPref.set('/saved/home')

    useExplorerData()
    await vi.waitFor(() => expect(loadMock).toHaveBeenCalledWith('/saved/home'))
  })

  it('preserves the default Home fallback when there is no argument or preference', async () => {
    const { loadMock } = installExplorerStateMock('')

    useExplorerData()
    await vi.waitFor(() => expect(loadMock).toHaveBeenCalledWith(undefined))
  })

  it('keeps an explicitly supplied initialPath ahead of process arguments', async () => {
    const { loadMock } = installExplorerStateMock('')
    getStartupPathMock.mockResolvedValue('/launch/folder')

    useExplorerData({ initialPath: '/explicit/folder' })
    await vi.waitFor(() => expect(loadMock).toHaveBeenCalledWith('/explicit/folder'))
    expect(getStartupPathMock).not.toHaveBeenCalled()
  })

  it('shows an invalid launch target error without silently loading the saved folder', async () => {
    const { loadMock, startDirPref, error } = installExplorerStateMock('')
    startDirPref.set('/saved/home')
    getStartupPathMock.mockRejectedValue({ code: 'invalid_input', message: 'Invalid start folder address' })
    let message = ''
    const unsubscribe = error.subscribe((value) => { message = value })

    useExplorerData()
    await vi.waitFor(() => expect(message).toBe('Invalid start folder address'))
    expect(loadMock).not.toHaveBeenCalled()
    await vi.waitFor(() => expect(eventHandlers.has('cloud-dir-refreshed')).toBe(true))
    unsubscribe()
  })

  it('does not navigate when disposed while the startup command is pending', async () => {
    const { loadMock } = installExplorerStateMock('')
    let resolvePath!: (path: string) => void
    getStartupPathMock.mockReturnValue(new Promise<string>((resolve) => { resolvePath = resolve }))

    useExplorerData()
    await vi.waitFor(() => expect(getStartupPathMock).toHaveBeenCalledTimes(1))
    cleanups.splice(0).forEach((cleanup) => cleanup())
    resolvePath('/late/folder')
    await Promise.resolve()
    await Promise.resolve()
    expect(loadMock).not.toHaveBeenCalled()
  })

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

  const cameraPath = '/run/user/1000/gvfs/mtp:host=test/DCIM/Camera'

  it('does not let a local watcher refresh replace pending user navigation before Loading becomes visible', async () => {
    vi.useFakeTimers()
    const { loadMock } = installExplorerStateMock('/dest')
    const hook = useExplorerData()
    await vi.advanceTimersByTimeAsync(0)
    loadMock.mockClear()
    let finish!: () => void
    loadMock.mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve }))
    const navigation = hook.load('/next')
    eventHandlers.get('dir-changed')?.({ payload: '/dest' })
    await vi.advanceTimersByTimeAsync(300)
    expect(loadMock).toHaveBeenCalledExactlyOnceWith('/next', undefined)
    finish()
    await navigation
    eventHandlers.get('dir-changed')?.({ payload: '/dest' })
    await vi.advanceTimersByTimeAsync(300)
    expect(loadMock).toHaveBeenCalledTimes(2)
  })

  it('coalesces GVFS polling and watcher notifications while a refresh is in flight', async () => {
    vi.useFakeTimers()
    const { loadMock } = installExplorerStateMock(cameraPath)
    useExplorerData()
    await vi.advanceTimersByTimeAsync(0)
    loadMock.mockClear()
    let finish!: () => void
    loadMock.mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve }))
    await vi.advanceTimersByTimeAsync(5000)
    eventHandlers.get('dir-changed')?.({ payload: cameraPath })
    await vi.advanceTimersByTimeAsync(5300)
    expect(loadMock).toHaveBeenCalledExactlyOnceWith(cameraPath, { recordHistory: false, silent: true })
    finish()
    await vi.advanceTimersByTimeAsync(0)
    eventHandlers.get('dir-changed')?.({ payload: cameraPath })
    await vi.advanceTimersByTimeAsync(300)
    expect(loadMock).toHaveBeenCalledTimes(2)
  })

  it.each(['loading', 'searchMode'] as const)('does not let a local watcher interrupt %s and resumes after it ends', async (blockedBy) => {
    vi.useFakeTimers()
    const fixture = installExplorerStateMock('/dest')
    useExplorerData()
    await vi.advanceTimersByTimeAsync(0)
    fixture.loadMock.mockClear()
    fixture[blockedBy].set(true)
    eventHandlers.get('dir-changed')?.({ payload: '/dest' })
    await vi.advanceTimersByTimeAsync(300)
    expect(fixture.loadMock).not.toHaveBeenCalled()
    fixture[blockedBy].set(false)
    eventHandlers.get('dir-changed')?.({ payload: '/dest' })
    await vi.advanceTimersByTimeAsync(300)
    expect(fixture.loadMock).toHaveBeenCalledExactlyOnceWith('/dest', { silent: true, recordHistory: false })
  })

  it('does not let GVFS polling interrupt navigation or active search', async () => {
    vi.useFakeTimers()
    const { loadMock, loading, searchMode } = installExplorerStateMock(cameraPath)
    useExplorerData()
    await vi.advanceTimersByTimeAsync(0)
    loadMock.mockClear()
    loading.set(true)
    eventHandlers.get('dir-changed')?.({ payload: cameraPath })
    await vi.advanceTimersByTimeAsync(5000)
    expect(loadMock).not.toHaveBeenCalled()
    loading.set(false)
    searchMode.set(true)
    await vi.advanceTimersByTimeAsync(5000)
    expect(loadMock).not.toHaveBeenCalled()
    searchMode.set(false)
    await vi.advanceTimersByTimeAsync(5000)
    expect(loadMock).toHaveBeenCalledOnce()
  })

  it('batches metadata events through the state reconciliation API and cancels pending work on cleanup', async () => {
    vi.useFakeTimers()
    const { applyEntryMetadata } = installExplorerStateMock(cameraPath)
    useExplorerData()
    await vi.advanceTimersByTimeAsync(0)
    const a: Entry = { name: 'a.jpg', path: `${cameraPath}/a.jpg`, kind: 'file', iconId: 0, size: 1 }
    eventHandlers.get('entry-meta')?.({ payload: a })
    eventHandlers.get('entry-meta-batch')?.({ payload: [{ ...a, size: 20 }] })
    await vi.advanceTimersByTimeAsync(50)
    expect(applyEntryMetadata).toHaveBeenCalledExactlyOnceWith([{ ...a, size: 20 }])
    eventHandlers.get('entry-meta')?.({ payload: a })
    cleanups.splice(0).forEach(cleanup => cleanup())
    await vi.advanceTimersByTimeAsync(10_000)
    expect(applyEntryMetadata).toHaveBeenCalledOnce()
    expect(eventHandlers.has('dir-changed')).toBe(false)
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
    await vi.advanceTimersByTimeAsync(0)
    expect(eventHandlers.has('volumes-changed')).toBe(true)
    loadPartitionsMock.mockClear()

    await vi.advanceTimersByTimeAsync(40)
    expect(loadPartitionsMock).toHaveBeenCalledTimes(1)

    mountsPollMs.set(120)
    await Promise.resolve()
    await vi.advanceTimersByTimeAsync(80)
    expect(loadPartitionsMock).toHaveBeenCalledTimes(1)

    await vi.advanceTimersByTimeAsync(40)
    expect(loadPartitionsMock).toHaveBeenCalledTimes(2)
  })

  it('debounces volume notifications even with polling disabled and cleans up listeners', async () => {
    vi.useFakeTimers()
    const { loadPartitionsMock } = installExplorerStateMock('~')
    useExplorerData()
    await vi.advanceTimersByTimeAsync(0)
    loadPartitionsMock.mockClear()
    const handler = eventHandlers.get('volumes-changed')
    expect(handler).toBeTypeOf('function')
    handler?.({ payload: null })
    handler?.({ payload: null })
    await vi.advanceTimersByTimeAsync(199)
    expect(loadPartitionsMock).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(loadPartitionsMock).toHaveBeenCalledOnce()
    handler?.({ payload: null })
    cleanups.splice(0).forEach((cleanup) => cleanup())
    expect(eventHandlers.has('volumes-changed')).toBe(false)
    await vi.advanceTimersByTimeAsync(200)
    expect(loadPartitionsMock).toHaveBeenCalledOnce()
  })

  it('starts cancellable activity for interactive cloud directory loads and hides it on success', async () => {
    const { loadDetailedMock } = installExplorerStateMock('~')
    const activityApi = {
      start: vi.fn(async () => {}),
      requestCancel: vi.fn(async () => {}),
      clearNow: vi.fn(),
      cleanup: vi.fn(async () => {}),
      hideSoon: vi.fn(),
      reportProgress: vi.fn(),
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
      reportProgress: vi.fn(),
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

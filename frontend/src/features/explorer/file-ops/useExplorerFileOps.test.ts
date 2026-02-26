import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => async () => {}),
}))

const previewCloudConflictsMock = vi.fn()
const listCloudEntriesMock = vi.fn()
const listCloudRemotesMock = vi.fn()
const copyCloudEntryMock = vi.fn()
const moveCloudEntryMock = vi.fn()

vi.mock('@/features/network', () => ({
  previewCloudConflicts: (...args: unknown[]) => previewCloudConflictsMock(...args),
  listCloudEntries: (...args: unknown[]) => listCloudEntriesMock(...args),
  listCloudRemotes: (...args: unknown[]) => listCloudRemotesMock(...args),
  copyCloudEntry: (...args: unknown[]) => copyCloudEntryMock(...args),
  moveCloudEntry: (...args: unknown[]) => moveCloudEntryMock(...args),
}))

const pasteClipboardPreviewMock = vi.fn()
const pasteClipboardCmdMock = vi.fn()
const setClipboardCmdMock = vi.fn()
const clearSystemClipboardMock = vi.fn()
const getSystemClipboardPathsMock = vi.fn()
const previewMixedTransferConflictsMock = vi.fn()
const copyMixedEntriesMock = vi.fn()
const moveMixedEntriesMock = vi.fn()

vi.mock('../services/clipboard.service', () => ({
  setClipboardCmd: (...args: unknown[]) => setClipboardCmdMock(...args),
  clearSystemClipboard: (...args: unknown[]) => clearSystemClipboardMock(...args),
  pasteClipboardCmd: (...args: unknown[]) => pasteClipboardCmdMock(...args),
  pasteClipboardPreview: (...args: unknown[]) => pasteClipboardPreviewMock(...args),
  getSystemClipboardPaths: (...args: unknown[]) => getSystemClipboardPathsMock(...args),
}))

vi.mock('../services/transfer.service', () => ({
  previewMixedTransferConflicts: (...args: unknown[]) => previewMixedTransferConflictsMock(...args),
  copyMixedEntries: (...args: unknown[]) => copyMixedEntriesMock(...args),
  moveMixedEntries: (...args: unknown[]) => moveMixedEntriesMock(...args),
}))

vi.mock('../services/files.service', () => ({
  entryKind: vi.fn(),
  dirSizes: vi.fn(),
  canExtractPaths: vi.fn(async () => false),
  extractArchive: vi.fn(),
  extractArchives: vi.fn(),
}))

vi.mock('../services/duplicates.service', () => ({
  checkDuplicatesStream: vi.fn(),
}))

vi.mock('../services/activity.service', () => ({
  cancelTask: vi.fn(),
}))

import { clipboardState, clearClipboardState, setClipboardPathsState } from './clipboard.store'
import { useExplorerFileOps } from './useExplorerFileOps'

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  requestCancel: (eventName: string) => Promise<void> | void
  hideSoon: () => void
  clearNow: () => void
  cleanup: (preserveTimer?: boolean) => Promise<void>
}

const activityApi: ActivityApi = {
  start: vi.fn(async () => {}),
  requestCancel: vi.fn(),
  hideSoon: vi.fn(),
  clearNow: vi.fn(),
  cleanup: vi.fn(async () => {}),
}

const createDeps = () => ({
  currentView: () => 'dir' as const,
  getCurrentPath: () => 'rclone://work/dest',
  clipboardMode: () => 'copy' as const,
  setClipboardPaths: vi.fn(),
  shouldOpenDestAfterExtract: () => false,
  loadPath: vi.fn(async () => {}),
  reloadCurrent: vi.fn(async () => {}),
  getDuplicateScanInput: () => ({ target: null, searchRoot: '', scanning: false }),
  duplicateModalStart: vi.fn(),
  duplicateModalSetProgress: vi.fn(),
  duplicateModalFinish: vi.fn(),
  duplicateModalFail: vi.fn(),
  duplicateModalStop: vi.fn(),
  duplicateModalClose: vi.fn(),
  showToast: vi.fn(),
  activityApi,
})

describe('useExplorerFileOps cloud conflict preview', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    clearClipboardState()
    setClipboardCmdMock.mockResolvedValue(undefined)
    clearSystemClipboardMock.mockResolvedValue(undefined)
    pasteClipboardCmdMock.mockResolvedValue(undefined)
    pasteClipboardPreviewMock.mockResolvedValue([])
    getSystemClipboardPathsMock.mockResolvedValue({ mode: 'copy', paths: [] })
    previewCloudConflictsMock.mockResolvedValue([])
    previewMixedTransferConflictsMock.mockResolvedValue([])
    copyMixedEntriesMock.mockResolvedValue([])
    moveMixedEntriesMock.mockResolvedValue([])
    listCloudEntriesMock.mockResolvedValue([])
    listCloudRemotesMock.mockResolvedValue([])
    copyCloudEntryMock.mockResolvedValue(undefined)
    moveCloudEntryMock.mockResolvedValue(undefined)
  })

  it('uses cloud conflict preview for cloud-to-cloud paste and opens conflict modal', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewCloudConflictsMock.mockResolvedValue([
      {
        src: 'rclone://work/src/report.txt',
        target: 'rclone://work/dest/report.txt',
        exists: true,
        isDir: false,
      },
    ])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

    expect(ok).toBe(false)
    expect(previewCloudConflictsMock).toHaveBeenCalledWith(
      ['rclone://work/src/report.txt'],
      'rclone://work/dest',
    )
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(get(fileOps.conflictModalOpen)).toBe(true)
    expect(get(fileOps.conflictList)).toEqual([
      {
        src: 'rclone://work/src/report.txt',
        target: 'rclone://work/dest/report.txt',
        is_dir: false,
      },
    ])
  })

  it('preserves directory conflict kind from cloud preview', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/Folder'])
    previewCloudConflictsMock.mockResolvedValue([
      {
        src: 'rclone://work/src/Folder',
        target: 'rclone://work/dest/Folder',
        exists: true,
        isDir: true,
      },
    ])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

    expect(ok).toBe(false)
    expect(get(fileOps.conflictList)).toEqual([
      {
        src: 'rclone://work/src/Folder',
        target: 'rclone://work/dest/Folder',
        is_dir: true,
      },
    ])
  })

  it('uses mixed conflict preview for local-to-cloud paste and opens conflict modal', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([
      {
        src: '/tmp/src/report.txt',
        target: 'rclone://work/dest/report.txt',
        exists: true,
        isDir: false,
      },
    ])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

    expect(ok).toBe(false)
    expect(previewMixedTransferConflictsMock).toHaveBeenCalledWith(
      ['/tmp/src/report.txt'],
      'rclone://work/dest',
    )
    expect(previewCloudConflictsMock).not.toHaveBeenCalled()
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(get(fileOps.conflictModalOpen)).toBe(true)
    expect(get(fileOps.conflictList)).toEqual([
      {
        src: '/tmp/src/report.txt',
        target: 'rclone://work/dest/report.txt',
        is_dir: false,
      },
    ])
  })

  it('uses mixed conflict preview for cloud-to-local paste and opens conflict modal', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([
      {
        src: 'rclone://work/src/report.txt',
        target: '/tmp/dest/report.txt',
        exists: true,
        isDir: false,
      },
    ])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(false)
    expect(previewMixedTransferConflictsMock).toHaveBeenCalledWith(
      ['rclone://work/src/report.txt'],
      '/tmp/dest',
    )
    expect(previewCloudConflictsMock).not.toHaveBeenCalled()
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(get(fileOps.conflictModalOpen)).toBe(true)
  })

  it('auto-renames on self-paste conflict in cloud and avoids local clipboard helpers', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewCloudConflictsMock.mockResolvedValue([
      {
        src: 'rclone://work/src/report.txt',
        target: 'rclone://work/src/report.txt',
        exists: true,
        isDir: false,
      },
    ])
    listCloudEntriesMock.mockResolvedValue([{ name: 'report.txt', path: '', kind: 'file' }])

    const deps = createDeps()
    deps.getCurrentPath = () => 'rclone://work/src'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/src')

    expect(ok).toBe(true)
    expect(copyCloudEntryMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      'rclone://work/src/report-1.txt',
      { overwrite: false, prechecked: true },
    )
    expect(previewCloudConflictsMock).toHaveBeenCalled()
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled()
  })

  it('treats OneDrive cloud rename-on-conflict set as case-insensitive', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewCloudConflictsMock.mockResolvedValue([
      {
        src: 'rclone://work/src/report.txt',
        target: 'rclone://work/src/report.txt',
        exists: true,
        isDir: false,
      },
    ])
    listCloudEntriesMock.mockResolvedValue([{ name: 'Report.txt', path: '', kind: 'file' }])
    listCloudRemotesMock.mockResolvedValue([
      {
        id: 'work',
        label: 'work (OneDrive)',
        provider: 'onedrive',
        rootPath: 'rclone://work',
        capabilities: {
          canList: true,
          canMkdir: true,
          canDelete: true,
          canRename: true,
          canMove: true,
          canCopy: true,
          canTrash: false,
          canUndo: false,
          canPermissions: false,
        },
      },
    ])

    const deps = createDeps()
    deps.getCurrentPath = () => 'rclone://work/src'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/src')

    expect(ok).toBe(true)
    expect(copyCloudEntryMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      'rclone://work/src/report-1.txt',
      { overwrite: false, prechecked: true },
    )
  })

  it('skips system clipboard sync on cloud destinations when pasting into current', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewCloudConflictsMock.mockResolvedValue([])
    listCloudEntriesMock.mockResolvedValue([])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.pasteIntoCurrent()

    expect(ok).toBe(true)
    expect(getSystemClipboardPathsMock).not.toHaveBeenCalled()
    expect(setClipboardCmdMock).not.toHaveBeenCalled()
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled()
    expect(copyCloudEntryMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      'rclone://work/dest/report.txt',
      { overwrite: false, prechecked: true },
    )
  })

  it('treats cloud paste as successful when refresh fails and shows refresh hint', async () => {
    vi.useFakeTimers()
    try {
      setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
      previewCloudConflictsMock.mockResolvedValue([])
      listCloudEntriesMock.mockResolvedValue([])

      const deps = createDeps()
      deps.reloadCurrent = vi.fn(async () => {
        throw new Error('Cloud operation timed out')
      })
      const fileOps = useExplorerFileOps(deps)

      const ok = await fileOps.pasteIntoCurrent()

      expect(ok).toBe(true)
      expect(copyCloudEntryMock).toHaveBeenCalledWith(
        'rclone://work/src/report.txt',
        'rclone://work/dest/report.txt',
        { overwrite: false, prechecked: true },
      )

      await vi.advanceTimersByTimeAsync(250)
      await Promise.resolve()

      expect(deps.showToast).toHaveBeenCalledWith(
        'Paste completed, but refresh took too long. Press F5 to refresh.',
        3500,
      )
      const toastCalls = (deps.showToast as unknown as { mock: { calls: unknown[][] } }).mock.calls
      expect(toastCalls.some((args) => String(args[0] ?? '').startsWith('Paste failed:'))).toBe(
        false,
      )
    } finally {
      vi.useRealTimers()
    }
  })

  it('coalesces cloud refresh requests for repeated paste into the same folder', async () => {
    vi.useFakeTimers()
    try {
      setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
      previewCloudConflictsMock.mockResolvedValue([])
      listCloudEntriesMock.mockResolvedValue([])

      const deps = createDeps()
      deps.reloadCurrent = vi.fn(async () => {})
      const fileOps = useExplorerFileOps(deps)

      const first = await fileOps.pasteIntoCurrent()
      const second = await fileOps.pasteIntoCurrent()

      expect(first).toBe(true)
      expect(second).toBe(true)
      expect(deps.reloadCurrent).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(250)
      await Promise.resolve()

      expect(deps.reloadCurrent).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('shows Moving… activity label for cloud cut paste', async () => {
    setClipboardPathsState('cut', ['rclone://work/src/report.txt'])
    previewCloudConflictsMock.mockResolvedValue([])
    listCloudEntriesMock.mockResolvedValue([])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

    expect(ok).toBe(true)
    expect(moveCloudEntryMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      'rclone://work/dest/report.txt',
      { overwrite: false, prechecked: true },
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^cloud-cut-/),
      expect.any(Function),
    )
  })

  it('executes local-to-cloud copy via mixed transfer command', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

    expect(ok).toBe(true)
    expect(copyMixedEntriesMock).toHaveBeenCalledWith(['/tmp/src/report.txt'], 'rclone://work/dest', {
      overwrite: false,
      prechecked: true,
    })
    expect(activityApi.start).toHaveBeenCalledWith(
      'Copying…',
      expect.stringMatching(/^mixed-copy-/),
      expect.any(Function),
    )
    expect(deps.reloadCurrent).not.toHaveBeenCalled()
  })

  it('executes cloud-to-local move via mixed transfer command and clears cut clipboard state', async () => {
    setClipboardPathsState('cut', ['rclone://work/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(true)
    expect(moveMixedEntriesMock).toHaveBeenCalledWith(['rclone://work/src/report.txt'], '/tmp/dest', {
      overwrite: false,
      prechecked: true,
    })
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^mixed-cut-/),
      expect.any(Function),
    )
    expect(deps.reloadCurrent).toHaveBeenCalledTimes(1)
    expect(get(clipboardState).mode).toBe('copy')
    expect(Array.from(get(clipboardState).paths)).toEqual([])
  })

  it('keeps mixed rename-on-conflict unsupported for execute until explicit target mapping is implemented', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([
      {
        src: '/tmp/src/report.txt',
        target: 'rclone://work/dest/report.txt',
        exists: true,
        isDir: false,
      },
    ])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')
    expect(ok).toBe(false)
    expect(get(fileOps.conflictModalOpen)).toBe(true)

    await fileOps.resolveConflicts('rename')

    expect(copyMixedEntriesMock).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith(
      'Rename on conflict for local/cloud paste is not supported yet',
    )
  })

  it('shows Moving… activity label for local cut paste', async () => {
    setClipboardPathsState('cut', ['/tmp/src/report.txt'])
    pasteClipboardPreviewMock.mockResolvedValue([])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(true)
    expect(pasteClipboardCmdMock).toHaveBeenCalledWith(
      '/tmp/dest',
      'rename',
      expect.stringMatching(/^cut-progress-/),
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^cut-progress-/),
      expect.any(Function),
    )
  })

  it('clears cut clipboard state after successful move via handlePasteOrMove', async () => {
    setClipboardPathsState('cut', ['/tmp/src/report.txt'])
    pasteClipboardPreviewMock.mockResolvedValue([])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(true)
    expect(get(clipboardState).mode).toBe('copy')
    expect(Array.from(get(clipboardState).paths)).toEqual([])
    expect(setClipboardCmdMock).toHaveBeenCalledWith([], 'copy')
    expect(clearSystemClipboardMock).toHaveBeenCalled()
  })

  it('prefers internal cut clipboard over system clipboard sync in pasteIntoCurrent', async () => {
    setClipboardPathsState('cut', ['/tmp/src/report.txt'])
    getSystemClipboardPathsMock.mockResolvedValue({
      mode: 'copy',
      paths: ['/tmp/other/file.txt'],
    })
    pasteClipboardPreviewMock.mockResolvedValue([])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.pasteIntoCurrent()

    expect(ok).toBe(true)
    expect(getSystemClipboardPathsMock).not.toHaveBeenCalled()
    expect(pasteClipboardCmdMock).toHaveBeenCalledWith(
      '/tmp/dest',
      'rename',
      expect.stringMatching(/^cut-progress-/),
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^cut-progress-/),
      expect.any(Function),
    )
  })
})

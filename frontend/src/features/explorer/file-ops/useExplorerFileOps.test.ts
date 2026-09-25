import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
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
const copyMixedEntryToMock = vi.fn()
const moveMixedEntryToMock = vi.fn()
const canExtractPathsMock = vi.fn<(_: string[]) => Promise<boolean>>(async (_paths: string[]) => false)
const extractArchiveMock = vi.fn<
  (_path: string, _progressEvent?: string) => Promise<
    | {
        destination: string
        skipped_symlinks: number
        skipped_entries: number
      }
    | undefined
  >
>(async (_path: string, _progressEvent?: string) => undefined)
const extractArchivesMock = vi.fn<
  (_paths: string[], _progressEvent?: string) => Promise<
    Array<{
      path?: string
      ok?: boolean
      result?: {
        destination: string
        skipped_symlinks: number
        skipped_entries: number
      } | null
      error?: string | null
    }>
  >
>(async (_paths: string[], _progressEvent?: string) => [])

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
  copyMixedEntryTo: (...args: unknown[]) => copyMixedEntryToMock(...args),
  moveMixedEntryTo: (...args: unknown[]) => moveMixedEntryToMock(...args),
}))

vi.mock('../services/files.service', () => ({
  entryKind: vi.fn(),
  dirSizes: vi.fn(),
  canExtractPaths: (paths: string[]) => canExtractPathsMock(paths),
  extractArchive: (path: string, progressEvent?: string) =>
    extractArchiveMock(path, progressEvent),
  extractArchives: (paths: string[], progressEvent?: string) =>
    extractArchivesMock(paths, progressEvent),
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

describe('useExplorerFileOps extract recovery', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    clearClipboardState()
    canExtractPathsMock.mockResolvedValue(true)
    extractArchiveMock.mockResolvedValue({
      destination: '/tmp/out',
      skipped_symlinks: 0,
      skipped_entries: 0,
    })
    extractArchivesMock.mockResolvedValue([])
  })

  it('surfaces extraction cancellation cleanly and clears activity state', async () => {
    extractArchiveMock.mockRejectedValueOnce(new Error('cancelled by user'))
    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp'
    const fileOps = useExplorerFileOps(deps)

    await fileOps.extractEntries([
      {
        name: 'archive.zip',
        path: '/tmp/archive.zip',
        kind: 'file',
        iconId: 0,
      },
    ])

    expect(canExtractPathsMock).toHaveBeenCalledWith(['/tmp/archive.zip'])
    expect(extractArchiveMock).toHaveBeenCalledTimes(1)
    expect(deps.reloadCurrent).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith('Extraction cancelled')
    expect(activityApi.clearNow).toHaveBeenCalledTimes(1)
    expect(activityApi.cleanup).toHaveBeenCalledTimes(1)
  })

  it('surfaces extraction failure cleanly and clears activity state', async () => {
    extractArchiveMock.mockRejectedValueOnce(new Error('Permission denied'))
    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp'
    const fileOps = useExplorerFileOps(deps)

    await fileOps.extractEntries([
      {
        name: 'archive.zip',
        path: '/tmp/archive.zip',
        kind: 'file',
        iconId: 0,
      },
    ])

    expect(canExtractPathsMock).toHaveBeenCalledWith(['/tmp/archive.zip'])
    expect(extractArchiveMock).toHaveBeenCalledTimes(1)
    expect(deps.reloadCurrent).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith('Failed to extract: Permission denied')
    expect(activityApi.clearNow).toHaveBeenCalledTimes(1)
    expect(activityApi.cleanup).toHaveBeenCalledTimes(1)
  })
})

describe('immutable paste and drop operations', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.clearAllMocks()
    clearClipboardState()
    pasteClipboardPreviewMock.mockResolvedValue([])
    previewCloudConflictsMock.mockResolvedValue([])
    previewMixedTransferConflictsMock.mockResolvedValue([])
    pasteClipboardCmdMock.mockResolvedValue(undefined)
    copyCloudEntryMock.mockResolvedValue(undefined)
    copyMixedEntriesMock.mockResolvedValue(undefined)
    clearSystemClipboardMock.mockResolvedValue(undefined)
  })

  afterEach(() => {
    vi.clearAllTimers()
    vi.useRealTimers()
  })

  it('copies an explicit local drop, never the previously cut cloud file', async () => {
    setClipboardPathsState('cut', ['rclone://work/old.jpg'])
    const previous = get(clipboardState)
    const ops = useExplorerFileOps(createDeps())
    await ops.handlePasteOrMove('/tmp/dest', { paths: ['/tmp/new.jpg'], mode: 'copy' })
    expect(pasteClipboardPreviewMock).toHaveBeenCalledWith('/tmp/dest', { paths: ['/tmp/new.jpg'], mode: 'copy' })
    expect(pasteClipboardCmdMock).toHaveBeenCalledWith('/tmp/dest', 'rename', expect.any(String), { paths: ['/tmp/new.jpg'], mode: 'copy' })
    expect(previewMixedTransferConflictsMock).not.toHaveBeenCalled()
    expect(moveMixedEntryToMock).not.toHaveBeenCalled()
    expect(get(clipboardState)).toBe(previous)
    expect(setClipboardCmdMock).not.toHaveBeenCalled()
    expect(clearSystemClipboardMock).not.toHaveBeenCalled()
  })

  it.each([
    ['local', '/tmp/src/a.txt', '/tmp/dest'],
    ['cloud', 'rclone://work/src/a.txt', 'rclone://work/dest'],
    ['local_to_cloud', '/tmp/src/a.txt', 'rclone://work/dest'],
    ['cloud_to_local', 'rclone://work/src/a.txt', '/tmp/dest'],
  ])('retains %s sources, destination and mode across a conflict dialog', async (route, src, dest) => {
    const conflicts = [{ src, target: `${dest}/a.txt`, is_dir: false, isDir: false }]
    pasteClipboardPreviewMock.mockResolvedValue(conflicts)
    previewCloudConflictsMock.mockResolvedValue(conflicts)
    previewMixedTransferConflictsMock.mockResolvedValue(conflicts)
    setClipboardPathsState('copy', [src])
    const deps = createDeps()
    const ops = useExplorerFileOps(deps)
    await ops.handlePasteOrMove(dest)
    expect(get(ops.conflictModalOpen)).toBe(true)
    setClipboardPathsState('cut', ['rclone://other/unrelated.txt'])
    const newer = get(clipboardState)
    deps.getCurrentPath = () => '/other/folder'
    await ops.resolveConflicts('overwrite')
    if (route === 'local') {
      expect(pasteClipboardCmdMock).toHaveBeenCalledWith(dest, 'overwrite', expect.any(String), { paths: [src], mode: 'copy' })
    } else if (route === 'cloud') {
      expect(copyCloudEntryMock).toHaveBeenCalledWith(src, `${dest}/a.txt`, expect.objectContaining({ overwrite: true }))
    } else {
      expect(copyMixedEntriesMock).toHaveBeenCalledWith([src], dest, expect.objectContaining({ overwrite: true }))
    }
    expect(moveCloudEntryMock).not.toHaveBeenCalled()
    expect(moveMixedEntriesMock).not.toHaveBeenCalled()
    expect(get(clipboardState)).toBe(newer)
    expect(clearSystemClipboardMock).not.toHaveBeenCalled()
    ops.cancelConflicts()
  })

  it('copies input before asynchronous preview and rejects an overlapping drop', async () => {
    let finish!: (items: unknown[]) => void
    pasteClipboardPreviewMock.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const ops = useExplorerFileOps(createDeps())
    const input: { paths: string[]; mode: 'copy' | 'cut' } = { paths: ['/tmp/original'], mode: 'copy' }
    const first = ops.handlePasteOrMove('/dest', input)
    input.paths[0] = '/tmp/changed'
    input.mode = 'cut'
    expect(await ops.handlePasteOrMove('/other', { paths: ['/tmp/other'], mode: 'cut' })).toBe(false)
    finish([])
    await first
    expect(pasteClipboardCmdMock).toHaveBeenCalledExactlyOnceWith('/dest', 'rename', expect.any(String), { paths: ['/tmp/original'], mode: 'copy' })
  })

  it('keeps the pending operation when a second drop arrives and retires it on cancel', async () => {
    pasteClipboardPreviewMock.mockResolvedValue([{ src: '/src/a', target: '/dest/a', is_dir: false }])
    const ops = useExplorerFileOps(createDeps())
    await ops.handlePasteOrMove('/dest', { paths: ['/src/a'], mode: 'copy' })
    expect(await ops.handlePasteOrMove('/other', { paths: ['/src/b'], mode: 'cut' })).toBe(false)
    expect(pasteClipboardPreviewMock).toHaveBeenCalledTimes(1)
    ops.cancelConflicts()
    await ops.resolveConflicts('overwrite')
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled()
  })

  it('executes a conflict confirmation only once and preserves a newer clipboard', async () => {
    pasteClipboardPreviewMock.mockResolvedValue([{ src: '/src/a', target: '/dest/a', is_dir: false }])
    let finish!: () => void
    pasteClipboardCmdMock.mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve }))
    setClipboardPathsState('cut', ['/src/a'])
    const ops = useExplorerFileOps(createDeps())
    await ops.handlePasteOrMove('/dest')
    const first = ops.resolveConflicts('overwrite')
    await vi.waitFor(() => expect(pasteClipboardCmdMock).toHaveBeenCalledTimes(1))
    await ops.resolveConflicts('overwrite')
    setClipboardPathsState('cut', ['/src/a']) // Even the same paths can be a new clipboard operation.
    const newer = get(clipboardState)
    finish()
    await first
    expect(pasteClipboardCmdMock).toHaveBeenCalledExactlyOnceWith('/dest', 'overwrite', expect.any(String), { paths: ['/src/a'], mode: 'cut' })
    expect(get(clipboardState)).toBe(newer)
    expect(clearSystemClipboardMock).not.toHaveBeenCalled()
  })

  it('never falls back to an old clipboard for an empty explicit drop', async () => {
    setClipboardPathsState('cut', ['/tmp/old'])
    const ops = useExplorerFileOps(createDeps())
    expect(await ops.handlePasteOrMove('/dest', { paths: [], mode: 'copy' })).toBe(false)
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled()
  })
})

describe('useExplorerFileOps local conflict preview', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    clearClipboardState()
    setClipboardCmdMock.mockResolvedValue(undefined)
    clearSystemClipboardMock.mockResolvedValue(undefined)
    pasteClipboardCmdMock.mockResolvedValue(undefined)
    pasteClipboardPreviewMock.mockResolvedValue([])
  })

  it('opens the local conflict modal when clipboard preview reports conflicts', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    pasteClipboardPreviewMock.mockResolvedValue([
      {
        src: '/tmp/src/report.txt',
        target: '/tmp/dest/report.txt',
        exists: true,
        is_dir: false,
      },
    ])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(false)
    expect(pasteClipboardPreviewMock).toHaveBeenCalledWith('/tmp/dest', {
      paths: ['/tmp/src/report.txt'], mode: 'copy',
    })
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled()
    expect(get(fileOps.conflictModalOpen)).toBe(true)
    expect(get(fileOps.conflictList)).toEqual([
      {
        src: '/tmp/src/report.txt',
        target: '/tmp/dest/report.txt',
        exists: true,
        is_dir: false,
      },
    ])
  })

  it('resolves local conflict modal through the explicit overwrite policy', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    pasteClipboardPreviewMock.mockResolvedValue([
      {
        src: '/tmp/src/report.txt',
        target: '/tmp/dest/report.txt',
        exists: true,
        is_dir: false,
      },
    ])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')
    expect(ok).toBe(false)
    expect(get(fileOps.conflictModalOpen)).toBe(true)

    await fileOps.resolveConflicts('overwrite')

    expect(pasteClipboardCmdMock).toHaveBeenCalledWith(
      '/tmp/dest',
      'overwrite',
      expect.stringMatching(/^copy-progress-/),
      { paths: ['/tmp/src/report.txt'], mode: 'copy' },
    )
    expect(get(fileOps.conflictModalOpen)).toBe(false)
    expect(get(fileOps.conflictList)).toEqual([])
  })
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
    copyMixedEntryToMock.mockResolvedValue('')
    moveMixedEntryToMock.mockResolvedValue('')
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
      expect.objectContaining({ overwrite: false, prechecked: true }),
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
      expect.objectContaining({ overwrite: false, prechecked: true }),
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
      expect.objectContaining({ overwrite: false, prechecked: true }),
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
        expect.objectContaining({ overwrite: false, prechecked: true }),
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
      expect.objectContaining({ overwrite: false, prechecked: true }),
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^cloud-cut-/),
      expect.any(Function),
    )
  })

  it('executes local-to-cloud copy via explicit mixed target command', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([])

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

    expect(ok).toBe(true)
    expect(copyMixedEntryToMock).toHaveBeenCalledWith(
      '/tmp/src/report.txt',
      'rclone://work/dest/report.txt',
      expect.objectContaining({ overwrite: false, prechecked: true }),
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Copying…',
      expect.stringMatching(/^mixed-copy-/),
      expect.any(Function),
    )
    expect(deps.reloadCurrent).not.toHaveBeenCalled()
  })

  it('executes cloud-to-local move via explicit mixed target command and clears cut clipboard state', async () => {
    setClipboardPathsState('cut', ['rclone://work/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(true)
    expect(moveMixedEntryToMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      '/tmp/dest/report.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^mixed-cut-/),
      expect.any(Function),
    )
    expect(deps.reloadCurrent).toHaveBeenCalledTimes(1)
    expect(get(clipboardState).mode).toBe('copy')
    expect(Array.from(get(clipboardState).paths)).toEqual([])
  })

  it('treats cloud-to-local refresh failure as soft failure after mixed write succeeds', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([])

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    deps.reloadCurrent = vi.fn(async () => {
      throw new Error('refresh failed')
    })
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(true)
    expect(copyMixedEntryToMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      '/tmp/dest/report.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
    )
    expect(deps.showToast).toHaveBeenCalledWith(
      'Paste completed, but refresh failed. Press F5 to refresh.',
      3500,
    )
  })

  it('refreshes cloud view after mixed local-to-cloud failure to reconcile partial writes', async () => {
    vi.useFakeTimers()
    try {
      setClipboardPathsState('copy', ['/tmp/src/a.txt', '/tmp/src/b.txt'])
      previewMixedTransferConflictsMock.mockResolvedValue([])
      copyMixedEntryToMock
        .mockResolvedValueOnce('rclone://work/dest/a.txt')
        .mockRejectedValueOnce(new Error('second source failed'))

      const deps = createDeps()
      deps.reloadCurrent = vi.fn(async () => {})
      const fileOps = useExplorerFileOps(deps)

      const ok = await fileOps.handlePasteOrMove('rclone://work/dest')

      expect(ok).toBe(false)
      expect(deps.reloadCurrent).not.toHaveBeenCalled()
      await vi.advanceTimersByTimeAsync(250)
      await Promise.resolve()
      expect(deps.reloadCurrent).toHaveBeenCalledTimes(1)
      expect(deps.showToast).toHaveBeenCalledWith('Paste failed: second source failed')
    } finally {
      vi.useRealTimers()
    }
  })

  it('attempts local refresh after mixed cloud-to-local failure to reconcile partial writes', async () => {
    setClipboardPathsState('cut', ['rclone://work/src/a.txt', 'rclone://work/src/b.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([])
    moveMixedEntryToMock
      .mockResolvedValueOnce('/tmp/dest/a.txt')
      .mockRejectedValueOnce(new Error('second source failed'))

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    deps.reloadCurrent = vi.fn(async () => {})
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(false)
    expect(deps.reloadCurrent).toHaveBeenCalledTimes(1)
    expect(deps.showToast).toHaveBeenCalledWith('Paste failed: second source failed')
  })

  it('resolves mixed local-to-cloud rename-on-conflict by retrying explicit target candidates', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([
      {
        src: '/tmp/src/report.txt',
        target: 'rclone://work/dest/report.txt',
        exists: true,
        isDir: false,
      },
    ])
    copyMixedEntryToMock
      .mockRejectedValueOnce({ code: 'destination_exists', message: 'exists' })
      .mockResolvedValueOnce('rclone://work/dest/report-1.txt')

    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/dest')
    expect(ok).toBe(false)
    expect(get(fileOps.conflictModalOpen)).toBe(true)

    await fileOps.resolveConflicts('rename')

    expect(copyMixedEntriesMock).not.toHaveBeenCalled()
    expect(copyMixedEntryToMock).toHaveBeenNthCalledWith(
      1,
      '/tmp/src/report.txt',
      'rclone://work/dest/report.txt',
      expect.objectContaining({
        overwrite: false,
        prechecked: true,
      }),
    )
    expect(copyMixedEntryToMock).toHaveBeenNthCalledWith(
      2,
      '/tmp/src/report.txt',
      'rclone://work/dest/report-1.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
    )
  })

  it('resolves mixed cloud-to-local rename-on-conflict for move by retrying target candidates', async () => {
    setClipboardPathsState('cut', ['rclone://work/src/report.txt'])
    previewMixedTransferConflictsMock.mockResolvedValue([
      {
        src: 'rclone://work/src/report.txt',
        target: '/tmp/dest/report.txt',
        exists: true,
        isDir: false,
      },
    ])
    moveMixedEntryToMock
      .mockRejectedValueOnce({ code: 'destination_exists', message: 'exists' })
      .mockResolvedValueOnce('/tmp/dest/report-1.txt')

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')
    expect(ok).toBe(false)
    expect(get(fileOps.conflictModalOpen)).toBe(true)

    await fileOps.resolveConflicts('rename')

    expect(moveMixedEntriesMock).not.toHaveBeenCalled()
    expect(moveMixedEntryToMock).toHaveBeenNthCalledWith(
      1,
      'rclone://work/src/report.txt',
      '/tmp/dest/report.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
    )
    expect(moveMixedEntryToMock).toHaveBeenNthCalledWith(
      2,
      'rclone://work/src/report.txt',
      '/tmp/dest/report-1.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
    )
  })

  it('avoids same-target overwrite when mixed cloud-to-local rename processes duplicate source leaf names', async () => {
    setClipboardPathsState('copy', [
      'rclone://work/srcA/report.txt',
      'rclone://work/srcB/report.txt',
    ])
    previewMixedTransferConflictsMock.mockResolvedValue([])
    copyMixedEntryToMock.mockResolvedValue('/tmp/dest/report.txt')

    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('/tmp/dest')

    expect(ok).toBe(true)
    expect(copyMixedEntryToMock).toHaveBeenNthCalledWith(
      1,
      'rclone://work/srcA/report.txt',
      '/tmp/dest/report.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
    )
    expect(copyMixedEntryToMock).toHaveBeenNthCalledWith(
      2,
      'rclone://work/srcB/report.txt',
      '/tmp/dest/report-1.txt',
      expect.objectContaining({ overwrite: false, prechecked: false }),
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
      { paths: ['/tmp/src/report.txt'], mode: 'cut' },
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
    expect(setClipboardCmdMock).not.toHaveBeenCalled()
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
      { paths: ['/tmp/src/report.txt'], mode: 'cut' },
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Moving…',
      expect.stringMatching(/^cut-progress-/),
      expect.any(Function),
    )
  })

  it('prefers internal copy clipboard over stale system clipboard sync in pasteIntoCurrent', async () => {
    setClipboardPathsState('copy', ['/tmp/src/report.txt'])
    getSystemClipboardPathsMock.mockResolvedValue({
      mode: 'cut',
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
      expect.stringMatching(/^copy-progress-/),
      { paths: ['/tmp/src/report.txt'], mode: 'copy' },
    )
    expect(activityApi.start).toHaveBeenCalledWith(
      'Copying…',
      expect.stringMatching(/^copy-progress-/),
      expect.any(Function),
    )
  })
})

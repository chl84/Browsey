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
  (_path: string, _progressEvent?: string, _password?: string) => Promise<
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
      error_code?: string | null
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
  extractArchive: (path: string, progressEvent?: string, password?: string) =>
    extractArchiveMock(path, progressEvent, password),
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
  start: (label: string, eventName: string, onCancel?: () => void, options?: { completeOnReply?: boolean }) => Promise<void>
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

  it('opens the returned cloud extraction folder without a local filesystem probe', async () => {
    const deps = createDeps()
    deps.shouldOpenDestAfterExtract = () => true
    const destination = 'rclone://work/archive-extract-unique'
    extractArchiveMock.mockResolvedValue({ destination, skipped_symlinks: 0, skipped_entries: 0 })
    await useExplorerFileOps(deps).extractEntries([
      { name: 'archive.zip', path: 'rclone://work/archive.zip', kind: 'file', iconId: 0 },
    ])
    expect(deps.loadPath).toHaveBeenCalledWith(destination, { recordHistory: true })
    expect(deps.activityApi.start).toHaveBeenCalledWith('Extracting…', expect.any(String), expect.any(Function), { completeOnReply: true })
  })

  it('prompts for passwords, retries wrong passwords, and forgets them afterwards', async () => {
    extractArchiveMock.mockRejectedValueOnce({ code: 'archive_password_required', message: 'Password required' })
      .mockRejectedValueOnce({ code: 'archive_invalid_password', message: 'Incorrect archive password' })
    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)
    const pending = fileOps.extractEntries([{ name: 'secret.zip', path: '/tmp/secret.zip', kind: 'file', iconId: 0 }])
    await vi.waitFor(() => expect(get(fileOps.archivePasswordModal.state).open).toBe(true))
    fileOps.archivePasswordModal.submit('wrong')
    await vi.waitFor(() => expect(get(fileOps.archivePasswordModal.state).error).toBe('Incorrect archive password'))
    fileOps.archivePasswordModal.submit(' good password ')
    await pending
    expect(extractArchiveMock.mock.calls.map((call) => call[2])).toEqual([undefined, 'wrong', ' good password '])
    expect(get(fileOps.archivePasswordModal.state)).toEqual({ open: false, path: '', error: '' })
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
  })

  it('cancels while waiting for a password without retrying extraction', async () => {
    extractArchiveMock.mockRejectedValueOnce({ code: 'archive_password_required', message: 'Password required' })
    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)
    const pending = fileOps.extractEntries([{ name: 'secret.zip', path: '/tmp/secret.zip', kind: 'file', iconId: 0 }])
    await vi.waitFor(() => expect(get(fileOps.archivePasswordModal.state).open).toBe(true))
    fileOps.cancelExtraction()
    await pending
    expect(extractArchiveMock).toHaveBeenCalledOnce()
    expect(deps.showToast).toHaveBeenCalledWith('Extraction cancelled')
    expect(get(fileOps.archivePasswordModal.state).open).toBe(false)
  })

  it('retries only encrypted batch failures and requests a separate password for each', async () => {
    extractArchivesMock.mockResolvedValueOnce([
      { path: '/tmp/plain.zip', ok: true, result: { destination: '/tmp/plain', skipped_symlinks: 0, skipped_entries: 0 } },
      { path: '/tmp/a.rar', ok: false, error: 'Password required', error_code: 'archive_password_required' },
      { path: '/tmp/b.7z', ok: false, error: 'Password required', error_code: 'archive_password_required' },
    ])
    const deps = createDeps()
    const fileOps = useExplorerFileOps(deps)
    const pending = fileOps.extractEntries(['plain.zip', 'a.rar', 'b.7z'].map((name) => ({ name, path: `/tmp/${name}`, kind: 'file', iconId: 0 })))
    await vi.waitFor(() => expect(get(fileOps.archivePasswordModal.state).path).toBe('/tmp/a.rar'))
    fileOps.archivePasswordModal.submit('first')
    await vi.waitFor(() => expect(get(fileOps.archivePasswordModal.state).path).toBe('/tmp/b.7z'))
    fileOps.archivePasswordModal.submit('second')
    await pending
    expect(extractArchiveMock.mock.calls.map((call) => [call[0], call[2]])).toEqual([['/tmp/a.rar', 'first'], ['/tmp/b.7z', 'second']])
    expect(deps.showToast).toHaveBeenCalledWith('Extracted 3 archives')
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
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
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
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(deps.showToast).toHaveBeenCalledWith('Failed to extract: Permission denied')
    expect(activityApi.clearNow).toHaveBeenCalledTimes(1)
    expect(activityApi.cleanup).toHaveBeenCalledTimes(1)
  })

  it('names failed batch archives and preserves completed and skipped counts without retrying', async () => {
    extractArchivesMock.mockResolvedValueOnce([
      { path: '/tmp/good.zip', ok: true, result: { destination: '/tmp/good', skipped_symlinks: 1, skipped_entries: 0 } },
      { path: '/tmp/bad.zip', ok: false, error: 'Invalid ZIP archive', error_code: 'archive_open_failed' },
    ])
    const deps = createDeps()
    const ops = useExplorerFileOps(deps)
    await ops.extractEntries(['good.zip', 'bad.zip'].map(name => ({ name, path: `/tmp/${name}`, kind: 'file', iconId: 0 })))
    expect(deps.showToast).toHaveBeenCalledWith(
      'Extracted 1 archive, 1 failed (skipped 1 symlink)\nbad.zip: Invalid ZIP archive', 12000,
    )
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(extractArchivesMock).toHaveBeenCalledOnce()
    expect(extractArchiveMock).not.toHaveBeenCalled()
    expect(activityApi.clearNow).toHaveBeenCalledOnce()
  })

  it('keeps the actual batch outcome when listing refresh fails', async () => {
    extractArchivesMock.mockResolvedValueOnce([
      { path: '/tmp/good.zip', ok: true, result: { destination: '/tmp/good', skipped_symlinks: 0, skipped_entries: 0 } },
      { path: '/tmp/bad.zip', ok: false, error: 'Invalid ZIP archive' },
    ])
    const deps = createDeps()
    deps.reloadCurrent.mockRejectedValueOnce(new Error('listing unavailable'))
    await useExplorerFileOps(deps).extractEntries(['good.zip', 'bad.zip'].map(name => ({ name, path: `/tmp/${name}`, kind: 'file', iconId: 0 })))
    expect(deps.showToast).toHaveBeenCalledWith(
      'Extracted 1 archive, 1 failed. Refresh failed. Press F5 to refresh.\nbad.zip: Invalid ZIP archive', 12000,
    )
    expect(extractArchivesMock).toHaveBeenCalledOnce()
  })

  it('does not label successful extraction as failed when listing refresh fails', async () => {
    const deps = createDeps()
    deps.reloadCurrent.mockRejectedValueOnce(new Error('listing unavailable'))
    await useExplorerFileOps(deps).extractEntries([{ name: 'good.zip', path: '/tmp/good.zip', kind: 'file', iconId: 0 }])
    expect(deps.showToast).toHaveBeenCalledWith('Extracted to /tmp/out. Refresh failed. Press F5 to refresh.', 12000)
    expect(extractArchiveMock).toHaveBeenCalledOnce()
  })

  it('refreshes after extraction errors without hiding the original failure or retrying', async () => {
    extractArchiveMock.mockRejectedValueOnce(new Error('Output retained at /tmp/partial'))
    const deps = createDeps()
    deps.reloadCurrent.mockRejectedValueOnce(new Error('listing unavailable'))
    await useExplorerFileOps(deps).extractEntries([{ name: 'bad.zip', path: '/tmp/bad.zip', kind: 'file', iconId: 0 }])
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(deps.showToast).toHaveBeenCalledWith(
      'Failed to extract: Output retained at /tmp/partial. Refresh failed. Press F5 to refresh.', 12000,
    )
    expect(extractArchiveMock).toHaveBeenCalledOnce()
  })

  it('bounds batch failure details while keeping the full failed count explicit', async () => {
    const names = ['a.zip', 'b.zip', 'c.zip', 'd.zip']
    extractArchivesMock.mockResolvedValueOnce(names.map(name => ({
      path: `/tmp/${name}`, ok: false, error: 'Invalid ZIP archive',
    })))
    const deps = createDeps()
    await useExplorerFileOps(deps).extractEntries(names.map(name => ({ name, path: `/tmp/${name}`, kind: 'file', iconId: 0 })))
    expect(deps.showToast).toHaveBeenCalledWith(
      'Extraction failed for 4 archives\na.zip: Invalid ZIP archive\nb.zip: Invalid ZIP archive\nc.zip: Invalid ZIP archive\n… 1 archive more failed', 12000,
    )
    expect(extractArchivesMock).toHaveBeenCalledOnce()
    expect(extractArchiveMock).not.toHaveBeenCalled()
  })

  it('keeps completed counts after mid-batch cancellation and warns separately about refresh', async () => {
    let finish!: (results: Awaited<ReturnType<typeof extractArchivesMock>>) => void
    extractArchivesMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const deps = createDeps()
    deps.reloadCurrent.mockRejectedValueOnce(new Error('listing unavailable'))
    const ops = useExplorerFileOps(deps)
    const pending = ops.extractEntries(['good.zip', 'pending.zip'].map(name => ({ name, path: `/tmp/${name}`, kind: 'file', iconId: 0 })))
    await vi.waitFor(() => expect(extractArchivesMock).toHaveBeenCalledOnce())
    ops.cancelExtraction()
    finish([
      { path: '/tmp/good.zip', ok: true, result: { destination: '/tmp/good', skipped_symlinks: 0, skipped_entries: 0 } },
      { path: '/tmp/pending.zip', ok: false, error: 'Extraction cancelled' },
    ])
    await pending
    expect(deps.showToast).toHaveBeenCalledWith(
      'Extracted 1 archive; extraction cancelled. Refresh failed. Press F5 to refresh.', 12000,
    )
    expect(extractArchiveMock).not.toHaveBeenCalled()
    expect(activityApi.requestCancel).toHaveBeenCalledOnce()
    expect(activityApi.clearNow).toHaveBeenCalledOnce()
    expect(activityApi.cleanup).toHaveBeenCalledOnce()
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

  it.each([
    ['/fixture/gvfs/server/source.bin', '/tmp/dest'],
    ['/tmp/source.bin', '/fixture/gvfs/server/dest'],
  ])('keeps %s transfer busy until cancellation is acknowledged without retry', async (source, target) => {
    const deps = createDeps()
    const ops = useExplorerFileOps(deps)
    let rejectCopy!: (error: unknown) => void
    pasteClipboardCmdMock.mockImplementationOnce(() => new Promise((_resolve, reject) => {
      rejectCopy = reject
    }))
    const pending = ops.handlePasteOrMove(target, { paths: [source], mode: 'copy' })
    await vi.waitFor(() => expect(pasteClipboardCmdMock).toHaveBeenCalledOnce())
    const eventName = pasteClipboardCmdMock.mock.calls[0][2]
    const onCancel = vi.mocked(activityApi.start).mock.calls[0][2]
    expect(onCancel).toBeTypeOf('function')
    onCancel?.()
    expect(activityApi.requestCancel).toHaveBeenCalledWith(eventName)
    expect(activityApi.cleanup).not.toHaveBeenCalled()
    expect(await ops.handlePasteOrMove(target, { paths: [source], mode: 'copy' })).toBe(false)
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
    rejectCopy({ code: 'cancelled', message: 'Copy cancelled; partial output retained; source not removed' })
    expect(await pending).toBe(false)
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(activityApi.clearNow).toHaveBeenCalledOnce()
    expect(activityApi.cleanup).toHaveBeenLastCalledWith(true)
    expect(deps.showToast).toHaveBeenLastCalledWith('Paste failed: Copy cancelled; partial output retained; source not removed')
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
  })

  it('releases successful local paste listeners without cancelling the completion timer', async () => {
    const deps = createDeps()
    const ops = useExplorerFileOps(deps)
    expect(await ops.handlePasteOrMove('/tmp/dest', { paths: ['/tmp/source'], mode: 'copy' })).toBe(true)
    expect(activityApi.hideSoon).toHaveBeenCalledOnce()
    expect(activityApi.cleanup).toHaveBeenCalledExactlyOnceWith(true)
    expect(activityApi.clearNow).not.toHaveBeenCalled()
  })

  it('does not retry or report successful paste as failed when listener cleanup fails', async () => {
    const deps = createDeps()
    vi.mocked(activityApi.cleanup).mockRejectedValueOnce(new Error('listener unavailable'))
    const ops = useExplorerFileOps(deps)
    expect(await ops.handlePasteOrMove('/tmp/dest', { paths: ['/tmp/source'], mode: 'copy' })).toBe(true)
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
    expect(deps.showToast).not.toHaveBeenCalled()
  })

  it('refreshes after local paste failure, keeps the clipboard and never retries', async () => {
    setClipboardPathsState('cut', ['/src/file.txt'])
    const clipboard = get(clipboardState)
    pasteClipboardCmdMock.mockRejectedValueOnce({ code: 'io_error', message: 'Copy retained at /dest/file.txt; source not removed' })
    const deps = createDeps()
    const ops = useExplorerFileOps(deps)
    expect(await ops.handlePasteOrMove('/dest')).toBe(false)
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
    expect(get(clipboardState)).toBe(clipboard)
    expect(clearSystemClipboardMock).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith('Paste failed: Copy retained at /dest/file.txt; source not removed')
  })

  it('does not report a successful local move as failed when listing refresh fails', async () => {
    setClipboardPathsState('cut', ['/src/file.txt'])
    const deps = createDeps()
    deps.reloadCurrent.mockRejectedValueOnce(new Error('refresh unavailable'))
    const ops = useExplorerFileOps(deps)
    expect(await ops.handlePasteOrMove('/dest')).toBe(true)
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
    expect(get(clipboardState).paths.size).toBe(0)
    expect(deps.showToast).toHaveBeenCalledWith('Paste completed, but refresh failed. Press F5 to refresh.', 3500)
  })

  it('keeps the original local operation error when recovery refresh also fails', async () => {
    pasteClipboardCmdMock.mockRejectedValueOnce({ code: 'rollback_failed', message: 'Rollback also failed; retained /dest' })
    const deps = createDeps()
    deps.reloadCurrent.mockRejectedValueOnce(new Error('refresh unavailable'))
    const ops = useExplorerFileOps(deps)
    expect(await ops.handlePasteOrMove('/dest', { paths: ['/src/file'], mode: 'copy' })).toBe(false)
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(deps.showToast).toHaveBeenCalledWith('Paste failed: Rollback also failed; retained /dest. Refresh also failed. Press F5 to refresh.')
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
  })

  it('preserves local failure and refreshes even if progress listener cleanup fails', async () => {
    pasteClipboardCmdMock.mockRejectedValueOnce(new Error('source changed; copies retained'))
    const deps = createDeps()
    vi.mocked(deps.activityApi.cleanup).mockRejectedValueOnce(new Error('listener unavailable'))
    const ops = useExplorerFileOps(deps)
    expect(await ops.handlePasteOrMove('/dest', { paths: ['/src/file'], mode: 'copy' })).toBe(false)
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(deps.showToast).toHaveBeenCalledWith('Paste failed: source changed; copies retained')
    expect(pasteClipboardCmdMock).toHaveBeenCalledOnce()
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

  it('stays busy through asynchronous preview and a late backend failure', async () => {
    let resolvePreview!: (conflicts: never[]) => void
    let rejectPaste!: (error: Error) => void
    const preview = new Promise<never[]>(resolve => { resolvePreview = resolve })
    const paste = new Promise<void>((_, reject) => { rejectPaste = reject })
    pasteClipboardPreviewMock.mockReturnValueOnce(preview)
    pasteClipboardCmdMock.mockReturnValueOnce(paste)
    setClipboardPathsState('copy', ['/tmp/src/tree'])
    const deps = createDeps()
    deps.getCurrentPath = () => '/tmp/dest'
    const ops = useExplorerFileOps(deps)
    const pending = ops.pasteIntoCurrent()
    expect(get(ops.pasteBusy)).toBe(true)
    expect(await ops.pasteIntoCurrent()).toBe(false)
    expect(pasteClipboardPreviewMock).toHaveBeenCalledTimes(1)
    resolvePreview([])
    await vi.waitFor(() => expect(pasteClipboardCmdMock).toHaveBeenCalledTimes(1))
    expect(get(ops.pasteBusy)).toBe(true)
    expect(activityApi.start).toHaveBeenCalledWith('Copying…', expect.any(String), expect.any(Function), { completeOnReply: true })
    rejectPaste(new Error('Late directory metadata failure'))
    expect(await pending).toBe(false)
    expect(deps.showToast).toHaveBeenCalledWith(expect.stringContaining('Late directory metadata failure'))
    expect(get(ops.pasteBusy)).toBe(false)
    expect(pasteClipboardCmdMock).toHaveBeenCalledTimes(1)
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
      { completeOnReply: true },
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
      { completeOnReply: true },
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
      { completeOnReply: true },
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
      { completeOnReply: true },
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
      { completeOnReply: true },
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
      { completeOnReply: true },
    )
  })
})

describe('conflict skip preserves immutable inputs and remaining cut sources', () => {
  beforeEach(() => {
    vi.clearAllMocks(); clearClipboardState()
    for (const mock of [pasteClipboardPreviewMock, previewCloudConflictsMock, previewMixedTransferConflictsMock,
      pasteClipboardCmdMock, copyCloudEntryMock, moveCloudEntryMock, copyMixedEntryToMock, moveMixedEntryToMock,
      listCloudEntriesMock, listCloudRemotesMock, clearSystemClipboardMock]) mock.mockReset()
    listCloudEntriesMock.mockResolvedValue([]); listCloudRemotesMock.mockResolvedValue([])
    pasteClipboardCmdMock.mockResolvedValue([]); copyCloudEntryMock.mockResolvedValue(undefined)
    moveCloudEntryMock.mockResolvedValue(undefined); copyMixedEntryToMock.mockResolvedValue(undefined)
    moveMixedEntryToMock.mockResolvedValue(undefined); clearSystemClipboardMock.mockResolvedValue(undefined)
  })

  it.each(['local', 'cloud', 'mixed'] as const)('skips only conflicting roots on %s and transfers the fresh root', async route => {
    const src = route === 'cloud' ? 'rclone://work/src' : '/src'
    const dest = route === 'local' ? '/dest' : 'rclone://work/dest'
    const deps = createDeps(); deps.getCurrentPath = () => dest
    setClipboardPathsState('copy', [`${src}/blocked`, `${src}/fresh`])
    const preview = { src: `${src}/blocked`, target: `${dest}/blocked`, is_dir: true, isDir: true }
    pasteClipboardPreviewMock.mockResolvedValue([preview]); previewCloudConflictsMock.mockResolvedValue([preview])
    previewMixedTransferConflictsMock.mockResolvedValue([preview])
    const ops = useExplorerFileOps(deps)
    await ops.handlePasteOrMove(dest); await ops.resolveConflicts('skip')
    const mock = route === 'local' ? pasteClipboardCmdMock : route === 'cloud' ? copyCloudEntryMock : copyMixedEntryToMock
    expect(mock).toHaveBeenCalledTimes(1)
    if (route === 'local') expect(mock).toHaveBeenCalledWith(dest, 'rename', expect.any(String), { mode: 'copy', paths: [`${src}/fresh`] })
    else expect(mock).toHaveBeenCalledWith(`${src}/fresh`, `${dest}/fresh`, expect.objectContaining({ overwrite: false }))
    expect(deps.showToast).toHaveBeenCalledWith('Skipped 1 conflicting item')
    expect(get(ops.conflictModalOpen)).toBe(false)
    expect([...get(clipboardState).paths]).toEqual([`${src}/blocked`, `${src}/fresh`])
  })

  it('keeps skipped cut sources on the clipboard and never clears a newer selection', async () => {
    for (const newer of [false, true]) {
      vi.clearAllMocks()
      const deps = createDeps(); deps.getCurrentPath = () => '/dest'
      setClipboardPathsState('cut', ['/src/blocked', '/src/fresh'])
      pasteClipboardPreviewMock.mockResolvedValue([{ src: '/src/blocked', target: '/dest/blocked', is_dir: true }])
      pasteClipboardCmdMock.mockImplementationOnce(async () => { if (newer) setClipboardPathsState('copy', ['/new/selection']); return [] })
      const ops = useExplorerFileOps(deps)
      await ops.handlePasteOrMove('/dest'); await ops.resolveConflicts('skip')
      expect([...get(clipboardState).paths]).toEqual(newer ? ['/new/selection'] : ['/src/blocked'])
      expect(clearSystemClipboardMock).toHaveBeenCalledTimes(newer ? 0 : 1)
    }
  })

  it('does not start a transfer or consume the cut clipboard when every root is skipped', async () => {
    const deps = createDeps(); deps.getCurrentPath = () => '/dest'
    setClipboardPathsState('cut', ['/src/blocked'])
    pasteClipboardPreviewMock.mockResolvedValue([{ src: '/src/blocked', target: '/dest/blocked', is_dir: false }])
    const ops = useExplorerFileOps(deps)
    await ops.handlePasteOrMove('/dest'); await ops.resolveConflicts('skip')
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled(); expect(deps.activityApi.start).not.toHaveBeenCalled()
    expect([...get(clipboardState).paths]).toEqual(['/src/blocked'])
    expect(deps.showToast).toHaveBeenCalledWith('Skipped 1 conflicting item')
  })
})

describe('unsafe paste and bounded unique names', () => {
  beforeEach(() => {
    vi.clearAllMocks(); clearClipboardState()
    for (const mock of [pasteClipboardPreviewMock, previewCloudConflictsMock, previewMixedTransferConflictsMock,
      pasteClipboardCmdMock, copyCloudEntryMock, moveCloudEntryMock, copyMixedEntryToMock, moveMixedEntryToMock,
      listCloudEntriesMock, listCloudRemotesMock]) mock.mockReset()
    listCloudEntriesMock.mockResolvedValue([]); listCloudRemotesMock.mockResolvedValue([])
    pasteClipboardPreviewMock.mockResolvedValue([]); previewCloudConflictsMock.mockResolvedValue([])
    previewMixedTransferConflictsMock.mockResolvedValue([])
  })
  it.each(['/same', 'rclone://work/same'])('rejects cut in the same parent before preview on %s', async dest => {
    const deps = createDeps(); deps.getCurrentPath = () => dest
    setClipboardPathsState('cut', [`${dest}/source`])
    const ops = useExplorerFileOps(deps); expect(await ops.handlePasteOrMove(dest)).toBe(false)
    expect(deps.showToast).toHaveBeenCalledWith('Paste failed: Source and destination are the same')
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled(); expect(previewCloudConflictsMock).not.toHaveBeenCalled()
    expect(deps.activityApi.start).not.toHaveBeenCalled(); expect([...get(clipboardState).paths]).toEqual([`${dest}/source`])
  })
  it('bounds exhausted cloud name reservation without dispatching a write', async () => {
    const deps = createDeps(); deps.getCurrentPath = () => 'rclone://work/dest'
    setClipboardPathsState('copy', ['rclone://work/src/a.txt'])
    listCloudEntriesMock.mockResolvedValue(Array.from({ length: 50 }, (_, i) => ({ name: i ? `a-${i}.txt` : 'a.txt' })))
    const ops = useExplorerFileOps(deps); await ops.handlePasteOrMove('rclone://work/dest')
    expect(copyCloudEntryMock).not.toHaveBeenCalled()
    expect(deps.showToast.mock.lastCall?.[0]).toContain('No available unique name after 50 candidates')
  })
  it('bounds cloud-to-local destination collisions and never retries an unknown write error', async () => {
    for (const collision of [true, false]) {
      vi.clearAllMocks(); copyMixedEntryToMock.mockReset()
      copyMixedEntryToMock.mockRejectedValue({ code: collision ? 'destination_exists' : 'io_error', message: 'failure' })
      const deps = createDeps(); deps.getCurrentPath = () => '/dest'
      setClipboardPathsState('copy', ['rclone://work/src/a.txt'])
      const ops = useExplorerFileOps(deps); await ops.handlePasteOrMove('/dest')
      expect(copyMixedEntryToMock).toHaveBeenCalledTimes(collision ? 50 : 1)
      expect([...get(clipboardState).paths]).toEqual(['rclone://work/src/a.txt'])
    }
  })
})

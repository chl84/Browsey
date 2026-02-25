import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => async () => {}),
}))

const previewCloudConflictsMock = vi.fn()
const statCloudEntryMock = vi.fn()
const copyCloudEntryMock = vi.fn()
const moveCloudEntryMock = vi.fn()

vi.mock('@/features/network', () => ({
  previewCloudConflicts: (...args: unknown[]) => previewCloudConflictsMock(...args),
  statCloudEntry: (...args: unknown[]) => statCloudEntryMock(...args),
  copyCloudEntry: (...args: unknown[]) => copyCloudEntryMock(...args),
  moveCloudEntry: (...args: unknown[]) => moveCloudEntryMock(...args),
}))

const pasteClipboardPreviewMock = vi.fn()
const pasteClipboardCmdMock = vi.fn()
const setClipboardCmdMock = vi.fn()
const clearSystemClipboardMock = vi.fn()
const getSystemClipboardPathsMock = vi.fn()

vi.mock('../services/clipboard.service', () => ({
  setClipboardCmd: (...args: unknown[]) => setClipboardCmdMock(...args),
  clearSystemClipboard: (...args: unknown[]) => clearSystemClipboardMock(...args),
  pasteClipboardCmd: (...args: unknown[]) => pasteClipboardCmdMock(...args),
  pasteClipboardPreview: (...args: unknown[]) => pasteClipboardPreviewMock(...args),
  getSystemClipboardPaths: (...args: unknown[]) => getSystemClipboardPathsMock(...args),
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

import { clearClipboardState, setClipboardPathsState } from './clipboard.store'
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
    statCloudEntryMock.mockResolvedValue(null)
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
    statCloudEntryMock
      .mockResolvedValueOnce({ path: 'rclone://work/src/report.txt', kind: 'file' })
      .mockResolvedValueOnce(null)

    const deps = createDeps()
    deps.getCurrentPath = () => 'rclone://work/src'
    const fileOps = useExplorerFileOps(deps)

    const ok = await fileOps.handlePasteOrMove('rclone://work/src')

    expect(ok).toBe(true)
    expect(copyCloudEntryMock).toHaveBeenCalledWith(
      'rclone://work/src/report.txt',
      'rclone://work/src/report-1.txt',
      { overwrite: false },
    )
    expect(previewCloudConflictsMock).toHaveBeenCalled()
    expect(pasteClipboardPreviewMock).not.toHaveBeenCalled()
    expect(pasteClipboardCmdMock).not.toHaveBeenCalled()
  })

  it('skips system clipboard sync on cloud destinations when pasting into current', async () => {
    setClipboardPathsState('copy', ['rclone://work/src/report.txt'])
    previewCloudConflictsMock.mockResolvedValue([])
    statCloudEntryMock.mockResolvedValue(null)

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
      { overwrite: false },
    )
  })
})

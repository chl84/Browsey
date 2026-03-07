import { describe, expect, it, vi } from 'vitest'
import { writable } from 'svelte/store'
import { createContextActions, type CurrentView } from './createContextActions'
import type { Entry } from '../model/types'
import type { ClipboardApi } from '../file-ops/createClipboard'

const moveToTrashManyMock = vi.fn(async (_paths: string[], _progressEvent?: string) => {})
const deleteEntriesMock = vi.fn(async (_paths: string[], _progressEvent?: string) => {})
const purgeTrashItemsMock = vi.fn(async (_ids: string[]) => {})
const restoreTrashItemsMock = vi.fn(async (_ids: string[]) => {})
const removeRecentMock = vi.fn(async (_paths: string[]) => {})
const copyPathsToSystemClipboardMock = vi.fn(async (_paths: string[], _mode?: string) => {})

vi.mock('../services/trash.service', () => ({
  moveToTrashMany: (paths: string[], progressEvent?: string) =>
    moveToTrashManyMock(paths, progressEvent),
  deleteEntries: (paths: string[], progressEvent?: string) =>
    deleteEntriesMock(paths, progressEvent),
  purgeTrashItems: (ids: string[]) => purgeTrashItemsMock(ids),
  restoreTrashItems: (ids: string[]) => restoreTrashItemsMock(ids),
  removeRecent: (paths: string[]) => removeRecentMock(paths),
}))

vi.mock('../services/clipboard.service', () => ({
  copyPathsToSystemClipboard: (paths: string[], mode?: string) =>
    copyPathsToSystemClipboardMock(paths, mode),
}))

const fileEntry = (path: string, name: string): Entry => ({
  path,
  name,
  kind: 'file',
  iconId: 0,
})

const makeClipboard = (): ClipboardApi => ({
  state: writable({ mode: 'copy', paths: new Set<string>() }),
  clear: vi.fn(),
  copy: vi.fn(async () => ({ ok: true as const })),
  copyPaths: vi.fn(async () => ({ ok: true as const })),
  cut: vi.fn(async () => ({ ok: true as const })),
  cutPaths: vi.fn(async () => ({ ok: true as const })),
  paste: vi.fn(async () => ({ ok: true as const })),
})

const createDeps = (entries: Entry[], selectedPaths: string[], view: CurrentView = 'dir') => ({
  getSelectedPaths: () => selectedPaths,
  getSelectedSet: () => new Set(selectedPaths),
  getFilteredEntries: () => entries,
  currentView: () => view,
  confirmDeleteEnabled: () => true,
  reloadCurrent: vi.fn(async () => {}),
  clipboard: makeClipboard(),
  showToast: vi.fn(),
  openWith: vi.fn(),
  startRename: vi.fn(),
  startAdvancedRename: vi.fn(),
  confirmDelete: vi.fn(),
  openProperties: vi.fn(),
  openLocation: vi.fn(async () => {}),
  openCompress: vi.fn(),
  openCheckDuplicates: vi.fn(),
  extractEntries: vi.fn(async () => {}),
})

describe('createContextActions', () => {
  it('routes move-trash to trash service (not permanent delete)', async () => {
    moveToTrashManyMock.mockClear()
    deleteEntriesMock.mockClear()
    const entry = fileEntry('/tmp/a.txt', 'a.txt')
    const deps = createDeps([entry], [entry.path], 'dir')
    const handle = createContextActions(deps)

    await handle('move-trash', entry)

    expect(moveToTrashManyMock).toHaveBeenCalledWith([entry.path], undefined)
    expect(deleteEntriesMock).not.toHaveBeenCalled()
  })

  it('routes delete-permanent to delete service when confirm modal is disabled', async () => {
    moveToTrashManyMock.mockClear()
    deleteEntriesMock.mockClear()
    const entry = fileEntry('/tmp/a.txt', 'a.txt')
    const deps = {
      ...createDeps([entry], [entry.path], 'dir'),
      confirmDeleteEnabled: () => false,
    }
    const handle = createContextActions(deps)

    await handle('delete-permanent', entry)

    expect(deleteEntriesMock).toHaveBeenCalledWith([entry.path], undefined)
    expect(moveToTrashManyMock).not.toHaveBeenCalled()
  })

  it('dispatches open-with action to dependency', async () => {
    const entry = fileEntry('/tmp/a.txt', 'a.txt')
    const deps = createDeps([entry], [entry.path])
    const handle = createContextActions(deps)

    await handle('open-with', entry)

    expect(deps.openWith).toHaveBeenCalledWith(entry)
  })

  it('dispatches rename action to dependency', async () => {
    const entry = fileEntry('/tmp/a.txt', 'a.txt')
    const deps = createDeps([entry], [entry.path])
    const handle = createContextActions(deps)

    await handle('rename', entry)

    expect(deps.startRename).toHaveBeenCalledWith(entry)
  })

  it('passes selected entries to advanced rename', async () => {
    const a = fileEntry('/tmp/a.txt', 'a.txt')
    const b = fileEntry('/tmp/b.txt', 'b.txt')
    const deps = createDeps([a, b], [a.path, b.path])
    const handle = createContextActions(deps)

    await handle('rename-advanced', a)

    expect(deps.startAdvancedRename).toHaveBeenCalledWith([a, b])
  })

  it('keeps local copy usable when system clipboard sync fails', async () => {
    copyPathsToSystemClipboardMock.mockRejectedValueOnce(new Error('xclip not found'))
    const entry = fileEntry('/tmp/a.txt', 'a.txt')
    const deps = createDeps([entry], [entry.path])
    const handle = createContextActions(deps)

    await handle('copy', entry)

    expect(deps.clipboard.copyPaths).toHaveBeenCalledWith([entry.path])
    expect(copyPathsToSystemClipboardMock).toHaveBeenCalledWith([entry.path], undefined)
    expect(deps.showToast).toHaveBeenCalledWith(
      'Copied (system clipboard unavailable: xclip not found)',
      2500,
    )
  })

  it('keeps local cut usable when system clipboard sync fails', async () => {
    copyPathsToSystemClipboardMock.mockRejectedValueOnce(new Error('xclip not found'))
    const entry = fileEntry('/tmp/a.txt', 'a.txt')
    const deps = createDeps([entry], [entry.path])
    const handle = createContextActions(deps)

    await handle('cut', entry)

    expect(deps.clipboard.cutPaths).toHaveBeenCalledWith([entry.path])
    expect(copyPathsToSystemClipboardMock).toHaveBeenCalledWith([entry.path], 'cut')
    expect(deps.showToast).toHaveBeenCalledWith(
      'Cut (system clipboard unavailable: xclip not found)',
      2500,
    )
  })

  it('shows a recovery toast and skips reload when restore fails in trash view', async () => {
    restoreTrashItemsMock.mockRejectedValueOnce(new Error('restore blocked'))
    const entry = { ...fileEntry('/tmp/a.txt', 'a.txt'), trash_id: 'trash-a' }
    const deps = createDeps([entry], [entry.path], 'trash')
    const handle = createContextActions(deps)

    await handle('restore', entry)

    expect(restoreTrashItemsMock).toHaveBeenCalledWith(['trash-a'])
    expect(deps.reloadCurrent).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith('Restore failed: restore blocked')
  })

  it('shows a recovery toast and skips reload when purge fails in trash view', async () => {
    purgeTrashItemsMock.mockRejectedValueOnce(new Error('purge blocked'))
    const entry = { ...fileEntry('/tmp/a.txt', 'a.txt'), trash_id: 'trash-a' }
    const deps = createDeps([entry], [entry.path], 'trash')
    const handle = createContextActions(deps)

    await handle('move-trash', entry)

    expect(purgeTrashItemsMock).toHaveBeenCalledWith(['trash-a'])
    expect(deps.reloadCurrent).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith('Delete failed: purge blocked')
  })
})

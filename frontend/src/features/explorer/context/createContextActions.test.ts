import { describe, expect, it, vi } from 'vitest'
import { writable } from 'svelte/store'
import { createContextActions, type CurrentView } from './createContextActions'
import type { Entry } from '../model/types'
import type { ClipboardApi } from '../file-ops/createClipboard'

const moveToTrashManyMock = vi.fn(async () => {})
const deleteEntriesMock = vi.fn(async () => {})
const purgeTrashItemsMock = vi.fn(async () => {})
const restoreTrashItemsMock = vi.fn(async () => {})
const removeRecentMock = vi.fn(async () => {})

vi.mock('../services/trash.service', () => ({
  moveToTrashMany: (...args: unknown[]) => moveToTrashManyMock(...args),
  deleteEntries: (...args: unknown[]) => deleteEntriesMock(...args),
  purgeTrashItems: (...args: unknown[]) => purgeTrashItemsMock(...args),
  restoreTrashItems: (...args: unknown[]) => restoreTrashItemsMock(...args),
  removeRecent: (...args: unknown[]) => removeRecentMock(...args),
}))

vi.mock('../services/clipboard.service', () => ({
  copyPathsToSystemClipboard: vi.fn(async () => {}),
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

    expect(moveToTrashManyMock).toHaveBeenCalledWith([entry.path])
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

    expect(deleteEntriesMock).toHaveBeenCalledWith([entry.path])
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
})

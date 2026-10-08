import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'
import { createExplorerFileActions } from './createExplorerFileActions'

const service = vi.hoisted(() => ({ openEntry: vi.fn(), openConsole: vi.fn() }))
vi.mock('../services/files.service', () => ({ openEntry: service.openEntry }))
vi.mock('../services/console.service', () => ({ openConsole: service.openConsole }))
const a: Entry = { path: '/mock/a', name: 'a', kind: 'file', iconId: 0 }
const b: Entry = { ...a, path: '/mock/b', name: 'b' }

const setup = (overrides: Partial<Parameters<typeof createExplorerFileActions>[0]> = {}) => {
  const deps = {
    selectionActions: { copy: vi.fn(), cut: vi.fn(), trash: vi.fn(), deletePermanently: vi.fn() },
    activityApi: { start: vi.fn(), requestCancel: vi.fn(), cleanup: vi.fn(), clearNow: vi.fn(), hideSoon: vi.fn(), hasHideTimer: () => false },
    currentView: (): CurrentView => 'dir', getCurrentPath: () => '/mock',
    getSelected: () => new Set([a.path]), getEntries: () => [a, b], getFilteredEntries: () => [a],
    pasteIntoCurrent: vi.fn(async () => true), startRename: vi.fn(), openProperties: vi.fn(), setSelection: vi.fn(), showToast: vi.fn(),
    ...overrides,
  }
  return { ...deps, actions: createExplorerFileActions(deps) }
}

describe('Explorer file-action orchestration', () => {
  beforeEach(() => vi.resetAllMocks())

  it('reads current selection for every copy/cut instead of capturing initial values', async () => {
    let selected = new Set([a.path])
    const { actions, selectionActions } = setup({ getSelected: () => selected })
    await actions.copy()
    selected = new Set([b.path])
    await actions.cut()
    expect(selectionActions.copy).toHaveBeenCalledWith([a.path])
    expect(selectionActions.cut).toHaveBeenCalledWith([b.path])
  })

  it('keeps permanent-delete confirmation policy in the shared controller', async () => {
    const { actions, selectionActions } = setup({ getSelected: () => new Set([a.path, b.path]) })
    await actions.delete()
    await actions.delete(true)
    await actions.deletePermanentFast()
    expect(selectionActions.trash).toHaveBeenCalledWith([a])
    expect(selectionActions.deletePermanently).toHaveBeenNthCalledWith(1, [a], true)
    expect(selectionActions.deletePermanently).toHaveBeenNthCalledWith(2, [a])
  })

  it.each(['network', 'recent', 'starred', 'trash'] as const)('does not paste in the %s view', async view => {
    const { actions, pasteIntoCurrent } = setup({ currentView: () => view })
    expect(await actions.paste()).toBe(false)
    expect(pasteIntoCurrent).not.toHaveBeenCalled()
  })

  it('pastes in directories through existing conflict/transfer orchestration', async () => {
    const { actions, pasteIntoCurrent } = setup()
    expect(await actions.paste()).toBe(true)
    expect(pasteIntoCurrent).toHaveBeenCalledOnce()
  })

  it('renames one existing selection only, never a connection root', () => {
    for (const paths of [[], [a.path, b.path], ['/missing']]) {
      const { actions, startRename } = setup({ getSelected: () => new Set(paths) })
      expect(actions.rename()).toBe(false)
      expect(startRename).not.toHaveBeenCalled()
    }
    const network = setup({ currentView: () => 'network' })
    expect(network.actions.rename()).toBe(false)
    expect(network.startRename).not.toHaveBeenCalled()
    const { actions, startRename } = setup()
    expect(actions.rename()).toBe(true)
    expect(startRename).toHaveBeenCalledWith(a)
  })

  it('opens properties for matching entries and ignores empty or stale selections', () => {
    const { actions, openProperties } = setup({ getSelected: () => new Set([a.path, b.path, '/missing']) })
    expect(actions.properties()).toBe(true)
    expect(openProperties).toHaveBeenCalledWith([a, b])
    for (const paths of [[], ['/missing']]) {
      const empty = setup({ getSelected: () => new Set(paths) })
      expect(empty.actions.properties()).toBe(false)
      expect(empty.openProperties).not.toHaveBeenCalled()
    }
  })

  it('anchors select-all to the filtered listing and resets anchors for a created item', () => {
    const { actions, setSelection } = setup({ getFilteredEntries: () => [a, b] })
    expect(actions.selectAll()).toBe(true)
    expect(setSelection).toHaveBeenLastCalledWith(new Set([a.path, b.path]), 0, 1)
    actions.selectCreated('/mock/new')
    expect(setSelection).toHaveBeenLastCalledWith(new Set(['/mock/new']), null, null)
    actions.selectCreated(null)
    expect(setSelection).toHaveBeenCalledTimes(2)
    const empty = setup({ getFilteredEntries: () => [] })
    expect(empty.actions.selectAll()).toBe(false)
    expect(empty.setSelection).not.toHaveBeenCalled()
  })

  it('opens local files and cloud directories without staging-file activity', async () => {
    const { actions, activityApi } = setup()
    await actions.open(a)
    const directory: Entry = { ...a, path: 'rclone://remote/folder', kind: 'dir' }
    await actions.open(directory)
    expect(service.openEntry).toHaveBeenNthCalledWith(1, a)
    expect(service.openEntry).toHaveBeenNthCalledWith(2, directory)
    expect(activityApi.start).not.toHaveBeenCalled()
    service.openEntry.mockRejectedValueOnce(new Error('local launch failed'))
    await expect(actions.open(a)).rejects.toThrow('local launch failed')
  })

  it('opens a cloud working copy with owned progress/cancel and releases its listener', async () => {
    const { actions, activityApi, showToast } = setup()
    const entry = { ...a, path: 'rclone://remote/photo.jpg' }
    await actions.open(entry)
    const [label, event, cancel] = vi.mocked(activityApi.start).mock.calls[0]
    expect(label).toBe('Opening cloud file…')
    expect(service.openEntry).toHaveBeenCalledWith(entry, { progressEvent: event })
    cancel?.()
    expect(activityApi.requestCancel).toHaveBeenCalledWith(event)
    expect(activityApi.hideSoon).toHaveBeenCalledOnce()
    expect(activityApi.cleanup).toHaveBeenCalledWith(true)
    expect(showToast).toHaveBeenCalledWith(expect.stringContaining('Cloud saves shows upload status'))
  })

  it.each(['start', 'open'])('reports cloud %s failure and cleans up without retry or success feedback', async phase => {
    const { actions, activityApi, showToast } = setup()
    if (phase === 'start') vi.mocked(activityApi.start).mockRejectedValueOnce(new Error('listener unavailable'))
    else service.openEntry.mockRejectedValueOnce(new Error('download failed'))
    await actions.open({ ...a, path: 'rclone://remote/file' })
    expect(activityApi.clearNow).toHaveBeenCalledOnce()
    expect(activityApi.cleanup).toHaveBeenCalledWith(false)
    expect(activityApi.hideSoon).not.toHaveBeenCalled()
    expect(showToast).toHaveBeenCalledWith(phase === 'start' ? 'listener unavailable' : 'download failed')
    expect(service.openEntry).toHaveBeenCalledTimes(phase === 'start' ? 0 : 1)
  })

  it('keeps console restrictions and errors distinct from opening files', async () => {
    const network = setup({ currentView: () => 'network' })
    expect(await network.actions.openConsole()).toBe(false)
    const cloud = setup({ getCurrentPath: () => 'rclone://remote/folder' })
    expect(await cloud.actions.openConsole()).toBe(true)
    expect(service.openConsole).not.toHaveBeenCalled()
    const local = setup()
    expect(await local.actions.openConsole()).toBe(true)
    expect(service.openConsole).toHaveBeenCalledWith('/mock')
    service.openConsole.mockRejectedValueOnce(new Error('console unavailable'))
    expect(await local.actions.openConsole()).toBe(false)
    expect(local.showToast).toHaveBeenCalledWith('Open console failed: console unavailable')
  })
})

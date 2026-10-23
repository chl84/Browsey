import { beforeEach, afterEach, it, expect, vi } from 'vitest'
import { get, writable } from 'svelte/store'
import { createActivity } from './hooks/createActivity'
import { createSelectionActions } from './file-ops/createSelectionActions'
const mocks = vi.hoisted(() => ({
  handlers: new Map<string,(event: {payload: unknown})=>void>(),
  move: vi.fn(), cancel: vi.fn(),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (name: string, handler: (event: {payload: unknown})=>void) => {
    mocks.handlers.set(name,handler)
    return () => { mocks.handlers.delete(name) }
  },
}))
vi.mock('./services/activity.service', () => ({ cancelTask: mocks.cancel }))
vi.mock('./services/trash.service', () => ({
  moveToTrashMany: mocks.move, deleteEntries: vi.fn(), purgeTrashItems: vi.fn(),
  needsNetworkDeleteConfirmation: () => false,
}))
vi.mock('./services/clipboard.service', () => ({copyPathsToSystemClipboard: vi.fn()}))
beforeEach(() => { vi.useFakeTimers(); vi.clearAllMocks(); mocks.handlers.clear(); mocks.move.mockResolvedValue(undefined) })
afterEach(() => vi.useRealTimers())
const actions = (activityApi: ReturnType<typeof createActivity>) => createSelectionActions({
  clipboard: {state: writable({mode:'copy',paths: new Set<string>()}),clear: vi.fn(),copy: vi.fn(),cut: vi.fn(),
    paste: vi.fn(),copyPaths: vi.fn(),cutPaths: vi.fn()},
  activityApi, currentView: () => 'dir', getCurrentPath: () => '/generated',
  confirmDeleteEnabled: () => false, confirmDelete: vi.fn(), reloadCurrent: vi.fn(async () => {}),
  showToast: vi.fn(),isWindows: () => false,
})
const entry = {path:'/generated/other.txt',name:'other.txt',kind:'file' as const,iconId:0}
it('restores the older paste after the real trash action releases its own activity', async () => {
  const api = createActivity()
  const cancel = vi.fn()
  await api.start('Long paste','paste-A',cancel)
  const oldReport = mocks.handlers.get('paste-A')!
  await actions(api).trash([entry])
  expect(mocks.move).toHaveBeenCalled()
  expect(mocks.handlers.has('paste-A')).toBe(true)
  await vi.advanceTimersByTimeAsync(1200)
  expect(get(api.activity)).toMatchObject({label:'Long paste',cancel})
  oldReport({payload:{bytes:1,total:2}})
  expect(get(api.activity)).toMatchObject({label:'Long paste',percent:50,cancel})
  get(api.activity)?.cancel?.()
  expect(cancel).toHaveBeenCalledOnce()
  await api.cleanupAll()
})
it('does not hide or unregister a newer paste when the older trash command completes', async () => {
  let finish!: () => void
  mocks.move.mockReturnValueOnce(new Promise<void>(resolve => { finish = resolve }))
  const api = createActivity()
  const pending = actions(api).trash([entry])
  await vi.advanceTimersByTimeAsync(0)
  expect(mocks.move).toHaveBeenCalledOnce()
  await api.start('New paste','paste-B',vi.fn())
  expect(mocks.handlers.has('paste-B')).toBe(true)
  finish()
  await pending
  expect(mocks.handlers.has('paste-B')).toBe(true)
  await vi.advanceTimersByTimeAsync(1200)
  expect(get(api.activity)).toMatchObject({label:'New paste',cancel:expect.any(Function)})
  await api.cleanupAll()
})

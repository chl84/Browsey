import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import CloudSaveStatus from './CloudSaveStatus.svelte'
import type { CloudWritebackStatus } from '@/features/network'

const { snapshot, listeners } = vi.hoisted(() => ({
  snapshot: vi.fn<() => Promise<CloudWritebackStatus[]>>(),
  listeners: new Map<string, Set<(event: { payload: unknown }) => void>>(),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (name: string, callback: (event: { payload: unknown }) => void) => {
    const handlers = listeners.get(name) ?? new Set()
    handlers.add(callback)
    listeners.set(name, handlers)
    return () => { handlers.delete(callback) }
  },
}))
vi.mock('@/features/network', async importOriginal => ({
  ...await importOriginal<typeof import('@/features/network')>(),
  cloudWritebackStatuses: snapshot,
}))

const components: ReturnType<typeof mount>[] = []
const row = (status: CloudWritebackStatus['status'], sequence: number, saveCompleted = false): CloudWritebackStatus => ({
  id: 'copy', name: 'file.txt', sourcePath: 'rclone://test/file.txt', status,
  sequence, saveCompleted, message: null, bytes: 0, total: 0,
})
const flush = async () => { for (let i = 0; i < 8; i++) { await Promise.resolve(); await tick() } }
const start = async () => {
  components.push(mount(CloudSaveStatus, { target: document.body }))
  await flush()
}
const emit = async (payload: CloudWritebackStatus) => {
  for (const handler of listeners.get('cloud-writeback') ?? []) handler({ payload })
  await flush()
}
const badge = () => document.querySelector('.cloud-save-status')
beforeEach(() => {
  vi.useFakeTimers()
  snapshot.mockResolvedValue([])
})
afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  listeners.clear()
  snapshot.mockReset()
  document.body.innerHTML = ''
  vi.useRealTimers()
})

it('keeps retained Saved snapshots and live unchanged-copy checks silent', async () => {
  // Even a previous completion in the initial snapshot is historical.
  snapshot.mockResolvedValue([row('saved', 1, true)])
  await start()
  expect(badge()).toBeNull()
  await emit(row('saved', 2))
  expect(badge()).toBeNull()
  await emit(row('pending', 3))
  expect(badge()?.textContent).toContain('Saving 1…')
  await emit(row('saved', 4))
  expect(badge()).toBeNull()
})

it('shows actual completions for four seconds without prolonging them on repeated state checks', async () => {
  await start()
  await emit(row('uploading', 1))
  expect(badge()?.textContent).toContain('Saving 1…')
  await emit(row('saved', 2, true))
  expect(badge()?.textContent).toContain('Saved')
  await vi.advanceTimersByTimeAsync(3000)
  await emit(row('saved', 3))
  await vi.advanceTimersByTimeAsync(1000)
  expect(badge()).toBeNull()
  await emit(row('saved', 4))
  expect(badge()).toBeNull()
  // A second actual completion revives the notice even without a progress event.
  await emit(row('saved', 5, true))
  expect(badge()?.textContent).toContain('Saved')
  await vi.advanceTimersByTimeAsync(4000)
  expect(badge()).toBeNull()
  await emit(row('saved', 5, true))
  expect(badge()).toBeNull()
})

it('does not lose a live completion when an older initial snapshot finishes later', async () => {
  let resolve!: (rows: CloudWritebackStatus[]) => void
  snapshot.mockReturnValue(new Promise(res => { resolve = res }))
  await start()
  await emit(row('saved', 3, true))
  resolve([row('saved', 1)])
  await flush()
  expect(badge()?.textContent).toContain('Saved')
  await vi.advanceTimersByTimeAsync(4000)
  expect(badge()).toBeNull()
})

it('keeps startup problems visible until dismissed and revives changed problems', async () => {
  snapshot.mockResolvedValue([row('conflict', 1)])
  await start()
  await vi.advanceTimersByTimeAsync(10000)
  expect(badge()?.textContent).toContain('1 need attention')
  document.querySelector<HTMLButtonElement>('[aria-label="Dismiss cloud save notification"]')!.click()
  await flush()
  expect(badge()).toBeNull()
  await emit(row('conflict', 2))
  expect(badge()).toBeNull()
  await emit({ ...row('error', 3), message: 'Network unavailable' })
  expect(badge()?.textContent).toContain('1 need attention')
  await emit(row('saved', 4))
  expect(badge()).toBeNull()
})

it('removes completion notices with deleted copies and ignores late completion events', async () => {
  await start()
  await emit(row('saved', 1, true))
  expect(badge()).not.toBeNull()
  for (const handler of listeners.get('cloud-working-copy-removed') ?? []) handler({ payload: 'copy' })
  await flush()
  expect(badge()).toBeNull()
  await emit(row('saved', 2, true))
  expect(badge()).toBeNull()
})

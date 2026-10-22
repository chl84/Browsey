import { get } from 'svelte/store'
import { beforeEach, expect, it, vi } from 'vitest'
import { createRenameModal } from './renameModal'
const service = vi.hoisted(() => ({ renameEntry: vi.fn() }))
vi.mock('../services/files.service', () => service)
beforeEach(() => service.renameEntry.mockReset())

it('does not start renaming after Cancel while progress setup is still pending', async () => {
  let finishSetup!: () => void
  const activityApi = {
    start: vi.fn(() => new Promise<void>(resolve => { finishSetup = resolve })),
    cleanup: vi.fn(async () => {}), clearNow: vi.fn(), hideSoon: vi.fn(), hasHideTimer: () => false,
  }
  const modal = createRenameModal({ loadPath: vi.fn(async () => {}), parentPath: () => '/owned', activityApi })
  modal.open({ name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 })
  const pending = modal.confirm('renamed.txt')
  modal.close()
  finishSetup()
  expect(await pending).toBe(false)
  expect(service.renameEntry).not.toHaveBeenCalled()
  expect(activityApi.cleanup).toHaveBeenCalledExactlyOnceWith(true)
})

it('preserves significant leading and trailing whitespace in a rename', async () => {
  service.renameEntry.mockResolvedValueOnce('/owned/ renamed.txt ')
  const modal = createRenameModal({ loadPath: vi.fn(async () => {}), parentPath: () => '/owned' })
  modal.open({ name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 })
  expect(await modal.confirm(' renamed.txt ')).toBe(true)
  expect(service.renameEntry).toHaveBeenLastCalledWith('/owned/old.txt', ' renamed.txt ')
})

it('clears a rejected rename error before a corrected request and suppresses repeated submission', async () => {
  service.renameEntry.mockRejectedValueOnce(new Error('Target already exists'))
  const modal = createRenameModal({ loadPath: vi.fn(async () => {}), parentPath: () => '/owned' })
  modal.open({ name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 })
  expect(await modal.confirm('occupied.txt')).toBe(false)
  expect(get(modal.state).error).toBe('Target already exists')
  let complete!: () => void
  service.renameEntry.mockImplementationOnce(() => new Promise<void>(resolve => { complete = resolve }))
  const pending = modal.confirm('corrected.txt')
  expect(get(modal.state)).toMatchObject({ open: true, error: '' })
  expect(await modal.confirm('duplicate.txt')).toBe(false)
  expect(service.renameEntry).toHaveBeenCalledTimes(2)
  complete(); expect(await pending).toBe(true)
  expect(get(modal.state)).toMatchObject({ open: false, error: '' })
})

it('reports a completed rename separately from a failed listing refresh', async () => {
  service.renameEntry.mockResolvedValueOnce('/owned/renamed.txt')
  const showToast = vi.fn()
  const modal = createRenameModal({
    loadPath: vi.fn(async () => { throw new Error('Listing unavailable') }),
    parentPath: () => '/owned', showToast,
  })
  modal.open({ name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 })
  expect(await modal.confirm('renamed.txt')).toBe(true)
  expect(get(modal.state)).toMatchObject({ open: false, error: '' })
  expect(showToast).toHaveBeenCalledWith('Rename completed, but refresh failed. Press F5 to refresh.', 3500)
  expect(service.renameEntry).toHaveBeenCalledOnce()
})

it.each([false, true])('keeps a reopened rename dialog intact when an older operation settles (failure=%s)', async failure => {
  let finish!: () => void
  service.renameEntry.mockReturnValueOnce(new Promise((resolve, reject) => {
    finish = () => failure ? reject(new Error('Old operation failed')) : resolve('/owned/renamed.txt')
  }))
  const showToast = vi.fn()
  const modal = createRenameModal({ loadPath: vi.fn(async () => {}), parentPath: () => '/owned', showToast })
  modal.open({ name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 })
  const pending = modal.confirm('renamed.txt')
  modal.close()
  modal.open({ name: 'next.txt', path: '/owned/next.txt', kind: 'file', iconId: 0 })
  finish()
  await pending
  expect(get(modal.state)).toMatchObject({ open: true, error: '', target: { path: '/owned/next.txt' } })
  if (failure) expect(showToast).toHaveBeenCalledWith('Rename failed: Old operation failed', 3500)
})

it('does not return to an old folder after a rename completes during navigation', async () => {
  let finish!: () => void
  service.renameEntry.mockReturnValueOnce(new Promise<void>(resolve => { finish = resolve }))
  let current = '/owned'
  const loadPath = vi.fn(async () => {})
  const modal = createRenameModal({ loadPath, parentPath: () => '/owned', getCurrentPath: () => current })
  modal.open({ name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 })
  const pending = modal.confirm('renamed.txt')
  modal.close()
  current = '/elsewhere'
  finish()
  expect(await pending).toBe(true)
  expect(loadPath).not.toHaveBeenCalled()
})

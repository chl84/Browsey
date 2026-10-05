import { get } from 'svelte/store'
import { expect, it, vi } from 'vitest'
import { createRenameModal } from './renameModal'
const service = vi.hoisted(() => ({ renameEntry: vi.fn() }))
vi.mock('../services/files.service', () => service)

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

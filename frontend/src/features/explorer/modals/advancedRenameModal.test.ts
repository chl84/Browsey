import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry } from '../model/types'

const previewRenameEntriesMock = vi.fn()
const renameEntriesMock = vi.fn()

vi.mock('../services/files.service', () => ({
  previewRenameEntries: (...args: unknown[]) => previewRenameEntriesMock(...args),
  renameEntries: (...args: unknown[]) => renameEntriesMock(...args),
}))

import { createAdvancedRenameModal } from './advancedRenameModal'

const makeEntry = (path: string, kind: Entry['kind'] = 'file'): Entry => ({
  name: path.split('/').pop() ?? path,
  path,
  kind,
  iconId: 0,
})

describe('advanced rename modal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    previewRenameEntriesMock.mockResolvedValue({
      rows: [{ original: 'report.txt', next: 'report.txt' }],
      error: null,
    })
    renameEntriesMock.mockResolvedValue(['/tmp/report.txt'])
  })

  it('keeps the modal open with an error when the rename request fails', async () => {
    renameEntriesMock.mockRejectedValueOnce(new Error('Permission denied'))
    const reloadCurrent = vi.fn(async () => {})
    const showToast = vi.fn()
    const modal = createAdvancedRenameModal({ reloadCurrent, showToast })

    modal.open([makeEntry('/tmp/report.txt')])
    await Promise.resolve()
    await Promise.resolve()

    const ok = await modal.confirm()

    expect(ok).toBe(false)
    expect(renameEntriesMock).toHaveBeenCalledWith([
      {
        path: '/tmp/report.txt',
        newName: 'report.txt',
      },
    ])
    expect(reloadCurrent).not.toHaveBeenCalled()
    expect(showToast).not.toHaveBeenCalled()
    expect(get(modal.state)).toMatchObject({
      open: true,
      error: 'Permission denied',
      previewLoading: false,
      previewError: '',
    })
  })

  it('keeps the modal open and surfaces preview failures before rename is attempted', async () => {
    previewRenameEntriesMock.mockRejectedValue(new Error('Preview failed'))
    const reloadCurrent = vi.fn(async () => {})
    const showToast = vi.fn()
    const modal = createAdvancedRenameModal({ reloadCurrent, showToast })

    modal.open([makeEntry('/tmp/report.txt')])
    await Promise.resolve()
    await Promise.resolve()

    const ok = await modal.confirm()

    expect(ok).toBe(false)
    expect(renameEntriesMock).not.toHaveBeenCalled()
    expect(reloadCurrent).not.toHaveBeenCalled()
    expect(showToast).not.toHaveBeenCalled()
    expect(get(modal.state)).toMatchObject({
      open: true,
      error: 'Preview failed',
      previewLoading: false,
      previewError: 'Preview failed',
    })
  })
})

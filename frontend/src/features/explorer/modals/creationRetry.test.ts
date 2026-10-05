import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { createNewFileModal } from './newFileModal'
import { createNewFolderModal } from './newFolderModal'

const services = vi.hoisted(() => ({ createFile: vi.fn(), createFolder: vi.fn() }))
vi.mock('../services/files.service', () => services)

describe('correcting a rejected creation draft', () => {
  it.each(['file', 'folder'] as const)('clears the stale %s error before awaiting a corrected request', async kind => {
    const create = kind === 'file' ? services.createFile : services.createFolder
    create.mockReset()
    create.mockRejectedValueOnce(new Error('Destination already exists'))
    const modal = (kind === 'file' ? createNewFileModal : createNewFolderModal)({
      getCurrentPath: () => '/generated/owned', loadPath: vi.fn(async () => {}), showToast: vi.fn(),
    })
    modal.open()
    expect(await modal.confirm('existing')).toBeNull()
    expect(get(modal.state)).toEqual({ open: true, error: 'Destination already exists' })
    let complete!: (path: string) => void
    create.mockImplementationOnce(() => new Promise<string>(resolve => { complete = resolve }))
    const corrected = modal.confirm('corrected')
    expect(get(modal.state)).toEqual({ open: true, error: '' })
    expect(await modal.confirm('duplicate-in-flight')).toBeNull()
    expect(create).toHaveBeenCalledTimes(2)
    complete('/generated/owned/corrected')
    expect(await corrected).toBe('/generated/owned/corrected')
    expect(get(modal.state)).toEqual({ open: false, error: '' })
  })
})

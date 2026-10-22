import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { createNewFileModal } from './newFileModal'
import { createNewFolderModal } from './newFolderModal'

const services = vi.hoisted(() => ({ createFile: vi.fn(), createFolder: vi.fn() }))
vi.mock('../services/files.service', () => services)

describe('correcting a rejected creation draft', () => {
  it('does not create a folder after Cancel during progress setup', async () => {
    services.createFolder.mockReset()
    let finishSetup!: () => void
    const activityApi = {
      start: vi.fn(() => new Promise<void>(resolve => { finishSetup = resolve })),
      cleanup: vi.fn(async () => {}), clearNow: vi.fn(), hideSoon: vi.fn(), hasHideTimer: () => false,
    }
    const modal = createNewFolderModal({
      getCurrentPath: () => '/generated/owned', loadPath: vi.fn(async () => {}), showToast: vi.fn(), activityApi,
    })
    modal.open()
    const pending = modal.confirm('cancelled')
    modal.close()
    finishSetup()
    expect(await pending).toBeNull()
    expect(services.createFolder).not.toHaveBeenCalled()
    expect(activityApi.cleanup).toHaveBeenCalledExactlyOnceWith(true, expect.stringMatching(/^mkdir-progress-/))
  })

  it.each(['file', 'folder'] as const)('keeps a completed %s creation successful when refresh fails', async kind => {
    const create = kind === 'file' ? services.createFile : services.createFolder
    create.mockReset().mockResolvedValueOnce('/generated/owned/created')
    const showToast = vi.fn()
    const modal = (kind === 'file' ? createNewFileModal : createNewFolderModal)({
      getCurrentPath: () => '/generated/owned',
      loadPath: vi.fn(async () => { throw new Error('Listing unavailable') }), showToast,
    })
    modal.open()
    expect(await modal.confirm('created')).toBe('/generated/owned/created')
    expect(get(modal.state)).toEqual({ open: false, error: '' })
    expect(showToast).toHaveBeenCalledWith(`${kind === 'file' ? 'File' : 'Folder'} created, but refresh failed. Press F5 to refresh.`)
    expect(create).toHaveBeenCalledOnce()
  })

  it.each([
    ['file', false], ['file', true], ['folder', false], ['folder', true],
  ] as const)('preserves a reopened %s dialog when an older creation settles (failure=%s)', async (kind, failure) => {
    const create = kind === 'file' ? services.createFile : services.createFolder
    let finish!: () => void
    create.mockReset().mockReturnValueOnce(new Promise<string>((resolve, reject) => {
      finish = () => failure ? reject(new Error('Old operation failed')) : resolve('/generated/owned/created')
    }))
    const showToast = vi.fn()
    const modal = (kind === 'file' ? createNewFileModal : createNewFolderModal)({
      getCurrentPath: () => '/generated/owned', loadPath: vi.fn(async () => {}), showToast,
    })
    modal.open()
    const pending = modal.confirm('created')
    modal.close()
    modal.open()
    finish()
    await pending
    expect(get(modal.state)).toEqual({ open: true, error: '' })
    if (failure) expect(showToast).toHaveBeenCalledWith(`${kind === 'file' ? 'File' : 'Folder'} creation failed: Old operation failed`)
  })

  it.each(['file', 'folder'] as const)('does not navigate back when %s creation finishes after leaving the folder', async kind => {
    const create = kind === 'file' ? services.createFile : services.createFolder
    let finish!: (path: string) => void
    create.mockReset().mockReturnValueOnce(new Promise<string>(resolve => { finish = resolve }))
    let current = '/generated/owned'
    const loadPath = vi.fn(async () => {})
    const modal = (kind === 'file' ? createNewFileModal : createNewFolderModal)({
      getCurrentPath: () => current, loadPath, showToast: vi.fn(),
    })
    modal.open()
    const pending = modal.confirm('created')
    modal.close()
    current = '/generated/elsewhere'
    finish('/generated/owned/created')
    expect(await pending).toBe('/generated/owned/created')
    expect(loadPath).not.toHaveBeenCalled()
  })

  it.each(['file', 'folder'] as const)('preserves significant whitespace in a new %s name', async kind => {
    const create = kind === 'file' ? services.createFile : services.createFolder
    create.mockReset().mockResolvedValueOnce('/generated/owned/ name ')
    const modal = (kind === 'file' ? createNewFileModal : createNewFolderModal)({
      getCurrentPath: () => '/generated/owned', loadPath: vi.fn(async () => {}), showToast: vi.fn(),
    })
    modal.open()
    expect(await modal.confirm(' name ')).toBe('/generated/owned/ name ')
    expect(create).toHaveBeenCalledExactlyOnceWith('/generated/owned', ' name ')
  })
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

import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Partition } from '../model/types'
import type { UsbFormatInfo, UsbFormatProgress, UsbFormatResult } from '../services/drives.service'
import { createUsbFormatModal } from './createUsbFormatModal'

const service = vi.hoisted(() => ({ inspect: vi.fn(), format: vi.fn() }))
vi.mock('../services/drives.service', async importOriginal => ({
  ...await importOriginal<typeof import('../services/drives.service')>(),
  getRemovableUsbFormatInfo: service.inspect,
  formatRemovablePartition: service.format,
}))

const drive: Partition = { label: 'Test USB', path: 'usb-volume://test-device', removable: true }
const info: UsbFormatInfo = {
  device: '/dev/test-device', model: 'Disposable test USB', sizeBytes: 32000000000,
  filesystems: [
    { id: 'ntfs', label: 'NTFS', description: '', available: false, requiredTool: 'mkfs.ntfs', labelMaxLength: 32 },
    { id: 'exfat', label: 'exFAT', description: '', available: true, requiredTool: 'mkfs.exfat', labelMaxLength: 15 },
    { id: 'fat32', label: 'FAT32', description: '', available: true, requiredTool: 'mkfs.fat', labelMaxLength: 11 },
  ],
}
const result: UsbFormatResult = {
  device: info.device, mountPath: '/mock/usb', sizeBytes: info.sizeBytes, filesystem: 'exfat', label: 'Test USB',
}
const setup = () => {
  const deps = { loadPartitions: vi.fn(async () => {}), reloadCurrent: vi.fn(async () => {}), openPath: vi.fn(), showToast: vi.fn() }
  return { ...deps, modal: createUsbFormatModal(deps) }
}
const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((r, fail) => { resolve = r; reject = fail })
  return { promise, resolve, reject }
}

describe('USB format controller', () => {
  beforeEach(() => {
    vi.resetAllMocks()
    service.inspect.mockResolvedValue(info)
    service.format.mockResolvedValue(result)
  })

  it('rejects system disks, phones and network mounts without inspecting or formatting', async () => {
    const { modal } = setup()
    for (const part of [
      { label: '/', path: '/', removable: false },
      { ...drive, path: 'mtp://phone', fs: 'mtp' },
      { ...drive, path: '/run/user/1000/gvfs/sftp:host=server' },
    ]) await modal.open(part)
    await modal.confirm()
    expect(get(modal.state).target).toBeNull()
    expect(service.inspect).not.toHaveBeenCalled()
    expect(service.format).not.toHaveBeenCalled()
  })

  it('selects the first available filesystem and discards inspection after closing', async () => {
    const { modal } = setup()
    await modal.open(drive)
    expect(get(modal.state).filesystem).toBe('exfat')
    const pending = deferred<UsbFormatInfo>()
    service.inspect.mockReturnValueOnce(pending.promise)
    const opening = modal.open(drive)
    modal.close()
    pending.resolve(info)
    await opening
    expect(get(modal.state)).toMatchObject({ target: null, info: null, error: '' })
  })

  it.each(['result', 'error'])('cannot replace a newer inspection with an old %s', async outcome => {
    const { modal } = setup()
    const pending = deferred<UsbFormatInfo>()
    service.inspect.mockReturnValueOnce(pending.promise)
    const first = modal.open(drive)
    const next = { ...drive, path: 'usb-volume://second' }
    await modal.open(next)
    if (outcome === 'result') pending.resolve({ ...info, model: 'Old result' })
    else pending.reject(new Error('Old error'))
    await first
    expect(get(modal.state).target).toEqual(next)
    expect(get(modal.state).info?.model).toBe(info.model)
    expect(get(modal.state).error).toBe('')
  })

  it('shows an inspection error and retries inspection without erasing', async () => {
    const { modal } = setup()
    service.inspect.mockRejectedValueOnce(new Error('device unavailable'))
    await modal.open(drive)
    expect(get(modal.state).error).toBe('USB inspection failed: device unavailable')
    await modal.confirm()
    expect(service.format).not.toHaveBeenCalled()
    await modal.retry()
    expect(get(modal.state).info).toEqual(info)
    expect(get(modal.state).error).toBe('')
    expect(service.format).not.toHaveBeenCalled()
  })

  it.each([
    { filesystem: 'ntfs' as const, label: '' },
    { filesystem: 'fat32' as const, label: 'too-long-label' },
    { filesystem: 'exfat' as const, label: 'invalid/name' },
  ])('guards unavailable utilities and invalid labels before erase: %j', async patch => {
    const { modal } = setup()
    await modal.open(drive)
    modal.state.update(s => ({ ...s, ...patch }))
    await modal.confirm()
    expect(service.format).not.toHaveBeenCalled()
  })

  it('locks close, repeat confirmation and switching drives until the format reply', async () => {
    const pending = deferred<UsbFormatResult>()
    let progress!: (value: UsbFormatProgress) => void
    service.format.mockImplementationOnce((_path, _fs, _label, callback) => { progress = callback; return pending.promise })
    const { modal, openPath, loadPartitions, reloadCurrent } = setup()
    await modal.open(drive)
    modal.state.update(s => ({ ...s, label: 'New USB' }))
    const formatting = modal.confirm()
    progress({ phase: 'Creating filesystem', percent: 45 })
    expect(get(modal.state)).toMatchObject({ busy: true, progress: { phase: 'Creating filesystem', percent: 45 } })
    modal.close()
    modal.openResult()
    await modal.open({ ...drive, path: 'usb-volume://other' })
    await modal.confirm()
    expect(service.format).toHaveBeenCalledTimes(1)
    expect(service.format).toHaveBeenCalledWith(drive.path, 'exfat', 'New USB', expect.any(Function))
    expect(service.inspect).toHaveBeenCalledTimes(1)
    expect(openPath).not.toHaveBeenCalled()
    pending.resolve(result)
    await formatting
    expect(get(modal.state)).toMatchObject({ result, busy: false, progress: null })
    expect(loadPartitions).toHaveBeenCalledWith({ forceNetworkRefresh: true })
    expect(reloadCurrent).toHaveBeenCalledOnce()
    await modal.confirm()
    expect(service.format).toHaveBeenCalledTimes(1)
    modal.openResult()
    expect(openPath).toHaveBeenCalledWith(result.mountPath)
    expect(get(modal.state).target).toBeNull()
    progress({ phase: 'Late', percent: 10 })
    expect(get(modal.state).progress).toBeNull()
  })

  it.each([
    ['format_status_unknown', 'Formatting status unknown'],
    ['format_busy', 'USB drive busy'],
    ['other', 'Format failed'],
  ])('preserves %s and requires fresh inspection before another erase', async (code, heading) => {
    const { modal, loadPartitions, reloadCurrent } = setup()
    service.format.mockRejectedValueOnce({ code, message: 'Original format error' })
    loadPartitions.mockRejectedValueOnce(new Error('refresh failed'))
    reloadCurrent.mockRejectedValueOnce(new Error('watch refresh failed'))
    await modal.open(drive)
    await modal.confirm()
    expect(get(modal.state)).toMatchObject({ error: `${heading}: Original format error`, busy: false, info: null, progress: null })
    await modal.confirm()
    expect(service.format).toHaveBeenCalledTimes(1)
    expect(reloadCurrent).toHaveBeenCalledOnce()
    await modal.retry()
    expect(get(modal.state).info).toEqual(info)
    expect(service.format).toHaveBeenCalledTimes(1)
  })

  it('preserves a successful erase when refresh fails and still repairs the directory watch', async () => {
    const { modal, loadPartitions, reloadCurrent, showToast } = setup()
    loadPartitions.mockRejectedValueOnce(new Error('refresh failed'))
    await modal.open(drive)
    await modal.confirm()
    expect(get(modal.state)).toMatchObject({ result, error: '', busy: false })
    expect(reloadCurrent).toHaveBeenCalledOnce()
    expect(showToast).toHaveBeenCalledWith('Could not refresh the drive listing. Press F5 to refresh.')
    expect(service.format).toHaveBeenCalledTimes(1)
  })

  it('ignores old progress even while a later formatting request is busy', async () => {
    let oldProgress!: (value: UsbFormatProgress) => void
    service.format.mockImplementationOnce(async (_path, _fs, _label, progress) => { oldProgress = progress; return result })
    const { modal } = setup()
    await modal.open(drive)
    await modal.confirm()
    modal.close()
    await modal.open({ ...drive, path: 'usb-volume://next' })
    const pending = deferred<UsbFormatResult>()
    service.format.mockReturnValueOnce(pending.promise)
    const formatting = modal.confirm()
    oldProgress({ phase: 'Old progress', percent: 99 })
    expect(get(modal.state).progress).toEqual({ phase: 'Checking USB drive', percent: null })
    pending.resolve(result)
    await formatting
  })
})

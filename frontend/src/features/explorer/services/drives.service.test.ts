import { describe, expect, it, vi } from 'vitest'
import { canFormatPartition, formatRemovablePartition, isMtpPartition, isUnmountedPartition } from './drives.service'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@/shared/lib/tauri', () => ({ invoke }))
vi.mock('@tauri-apps/api/core', () => ({ Channel: class { onmessage?: (value: unknown) => void } }))

describe('USB format progress', () => {
  it('forwards progress while running and ignores late messages after completion', async () => {
    const onProgress = vi.fn()
    let finish!: (result: unknown) => void
    invoke.mockImplementationOnce((_command, args) => {
      args.onProgress.onmessage({ phase: 'Creating filesystem', percent: 42 })
      return new Promise((resolve) => { finish = resolve })
    })
    const pending = formatRemovablePartition('/usb', 'ext4', 'TEST', onProgress)
    expect(onProgress).toHaveBeenCalledWith({ phase: 'Creating filesystem', percent: 42 })
    finish({ device: '/dev/test', filesystem: 'ext4' })
    await pending
    invoke.mock.calls.at(-1)?.[1].onProgress.onmessage({ phase: 'late', percent: 99 })
    expect(onProgress).toHaveBeenCalledOnce()
  })

  it('does not retry a destructive command after an uncertain result', async () => {
    invoke.mockClear()
    invoke.mockRejectedValueOnce({ code: 'format_status_unknown', message: 'Connection lost' })
    await expect(formatRemovablePartition('/usb', 'ext4', '')).rejects.toMatchObject({ code: 'format_status_unknown' })
    expect(invoke).toHaveBeenCalledOnce()
  })
})

describe('phone and USB capabilities', () => {
  it.each(['mtp://Phone_A/', '/run/user/1000/gvfs/mtp:host=Phone_A'])('never offers phone formatting (%s)', (path) => {
    const phone = { label: 'Phone', path, fs: 'mtp', removable: true }
    expect(isMtpPartition(phone)).toBe(true)
    expect(canFormatPartition(phone)).toBe(false)
  })
  it('distinguishes unmounted phones from mounted phones and real USB partitions', () => {
    expect(isUnmountedPartition('mtp://Phone_A/')).toBe(true)
    expect(isUnmountedPartition('/run/user/1000/gvfs/mtp:host=Phone_A')).toBe(false)
    expect(isUnmountedPartition('usb-volume:///dev/sdz1')).toBe(true)
    expect(canFormatPartition({ label: 'USB', path: '/media/USB', fs: 'ext4', removable: true })).toBe(true)
    expect(canFormatPartition({ label: 'USB', path: 'usb-volume:///dev/sdz1', removable: true })).toBe(true)
    expect(canFormatPartition({ label: 'NAS', path: '/run/user/1000/gvfs/smb-share:server=nas', removable: true })).toBe(false)
  })
})

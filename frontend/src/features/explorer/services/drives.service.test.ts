import { describe, expect, it, vi } from 'vitest'
import { formatRemovablePartition } from './drives.service'

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

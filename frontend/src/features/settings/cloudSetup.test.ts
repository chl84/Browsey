import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createDebouncedAsyncRunner,
  describeCloudSetupStatus,
  RCLONE_PATH_REFRESH_DEBOUNCE_MS,
} from './cloudSetup'

describe('cloud setup helpers', () => {
  beforeEach(() => {
    vi.useRealTimers()
  })

  it('describes the ready setup state for Settings Advanced', () => {
    const copy = describeCloudSetupStatus({
      state: 'ready',
      configuredPath: null,
      resolvedBinaryPath: '/usr/bin/rclone',
      detectedRemoteCount: 1,
      supportedRemoteCount: 1,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })

    expect(copy.headline).toBe('Rclone is ready')
    expect(copy.nextStep).toContain('Network')
  })

  it('does not run the debounced blur action on every keystroke', async () => {
    vi.useFakeTimers()
    const run = vi.fn(async (_value: string) => {})
    const runner = createDebouncedAsyncRunner(run)

    runner.schedule('/tmp/rclone-a')
    runner.schedule('/tmp/rclone-b')

    await vi.advanceTimersByTimeAsync(RCLONE_PATH_REFRESH_DEBOUNCE_MS - 1)
    expect(run).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1)
    expect(run).toHaveBeenCalledTimes(1)
    expect(run).toHaveBeenCalledWith('/tmp/rclone-b')
  })

  it('cancels a pending debounced blur refresh', async () => {
    vi.useFakeTimers()
    const run = vi.fn(async (_value: string) => {})
    const runner = createDebouncedAsyncRunner(run)

    runner.schedule('/tmp/rclone')
    runner.cancel()

    await vi.advanceTimersByTimeAsync(RCLONE_PATH_REFRESH_DEBOUNCE_MS)
    expect(run).not.toHaveBeenCalled()
  })
})

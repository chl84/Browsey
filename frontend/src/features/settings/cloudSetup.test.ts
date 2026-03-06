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

  it.each([
    [
      'ready',
      {
        headline: 'Rclone is ready',
        nextStepPart: 'Network',
      },
    ],
    [
      'binary_missing',
      {
        headline: 'Rclone was not found',
        nextStepPart: 'Install rclone',
      },
    ],
    [
      'invalid_binary_path',
      {
        headline: 'Rclone path is invalid',
        nextStepPart: 'valid executable path',
      },
    ],
    [
      'config_read_failed',
      {
        headline: 'Cloud setup could not be read',
        nextStepPart: 'saved rclone setting',
      },
    ],
    [
      'runtime_unusable',
      {
        headline: 'Rclone is installed but unusable',
        nextStepPart: 'supported version',
      },
    ],
    [
      'no_supported_remotes',
      {
        headline: 'No supported cloud remotes found',
        nextStepPart: 'rclone config',
      },
    ],
    [
      'discovery_failed',
      {
        headline: 'Cloud remotes could not be inspected',
        nextStepPart: 'try again later',
      },
    ],
  ] as const)('describes the %s setup state for Settings Cloud', (state, expected) => {
    const copy = describeCloudSetupStatus({
      state,
      configuredPath: null,
      resolvedBinaryPath: state === 'binary_missing' ? null : '/usr/bin/rclone',
      detectedRemoteCount: 1,
      supportedRemoteCount: state === 'ready' ? 1 : 0,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })

    expect(copy.headline).toBe(expected.headline)
    expect(copy.nextStep).toContain(expected.nextStepPart)
  })

  it('describes the pending/null setup state for Settings Cloud', () => {
    const copy = describeCloudSetupStatus(null)

    expect(copy.headline).toBe('Checking cloud setup…')
    expect(copy.nextStep).toContain('inspecting')
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

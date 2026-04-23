import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createDebouncedAsyncRunner,
  describeCloudProbeRecommendation,
  describeCloudProbeState,
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

  it('describes the probe recommendation copy', () => {
    const healthyCliOnly = describeCloudProbeRecommendation({
      remote: {
        id: 'browsey-gdrive',
        label: 'browsey-gdrive (Google Drive)',
        provider: 'gdrive',
        rootPath: 'rclone://browsey-gdrive',
        capabilities: {
          canList: true,
          canMkdir: true,
          canDelete: true,
          canRename: true,
          canMove: true,
          canCopy: true,
          canTrash: false,
          canUndo: false,
          canPermissions: false,
        },
      },
      rc: { ok: false, state: 'timeout', message: 'Timed out', elapsedMs: 1000 },
      cli: { ok: true, state: 'ok', message: 'Listed', elapsedMs: 1200 },
      recommendation: 'healthy_cli_only',
    })

    expect(healthyCliOnly.headline).toBe('Remote works via CLI only')
    expect(healthyCliOnly.nextStep).toContain('BROWSEY_RCLONE_RC=0')
  })

  it('describes compact probe state labels', () => {
    expect(describeCloudProbeState(true, 'ok')).toBe('OK')
    expect(describeCloudProbeState(false, 'auth_required')).toBe('Auth required')
    expect(describeCloudProbeState(false, 'task_failed')).toBe('Unavailable')
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

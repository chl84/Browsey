import type { CloudSetupStatus } from '@/features/network'

export const RCLONE_PATH_REFRESH_DEBOUNCE_MS = 180

export const describeCloudSetupStatus = (status: CloudSetupStatus | null) => {
  switch (status?.state) {
    case 'ready':
      return {
        headline: 'Rclone is ready',
        nextStep: 'Configured remotes are available in Network.',
      }
    case 'binary_missing':
      return {
        headline: 'Rclone was not found',
        nextStep: 'Install rclone or set Rclone path below.',
      }
    case 'invalid_binary_path':
      return {
        headline: 'Rclone path is invalid',
        nextStep: 'Clear the field to auto-detect or enter a valid executable path.',
      }
    case 'config_read_failed':
      return {
        headline: 'Cloud setup could not be read',
        nextStep: 'Browsey could not read the saved rclone setting. Reopen Settings or check the app data directory.',
      }
    case 'runtime_unusable':
      return {
        headline: 'Rclone is installed but unusable',
        nextStep: 'Update rclone to a supported version and try again.',
      }
    case 'no_supported_remotes':
      return {
        headline: 'No supported cloud remotes found',
        nextStep: 'Run `rclone config` and create a OneDrive, Google Drive, or Nextcloud remote.',
      }
    case 'discovery_failed':
      return {
        headline: 'Cloud remotes could not be inspected',
        nextStep: 'Check the rclone config and try again later.',
      }
    default:
      return {
        headline: 'Checking cloud setup…',
        nextStep: 'Browsey is inspecting the current rclone setup.',
      }
  }
}

export const createDebouncedAsyncRunner = <T>(
  run: (value: T) => Promise<void> | void,
  delayMs = RCLONE_PATH_REFRESH_DEBOUNCE_MS,
) => {
  let timer: ReturnType<typeof setTimeout> | null = null

  const cancel = () => {
    if (!timer) return
    clearTimeout(timer)
    timer = null
  }

  const schedule = (value: T) => {
    cancel()
    timer = setTimeout(() => {
      void run(value)
    }, delayMs)
  }

  return { schedule, cancel }
}

import { normalizeError } from '@/shared/lib/error'
import { invoke } from '@/shared/lib/tauri'

export type OpenWithApp = {
  id: string
  name: string
  comment?: string
  exec: string
  icon?: string
  matches: boolean
  terminal: boolean
}

export type OpenWithChoice = {
  appId?: string | null
}

const userOpenWithErrorMessage = (code: string | undefined, message: string) => {
  switch (code) {
    case 'app_not_found':
      return 'The selected application is unavailable'
    case 'launch_failed':
      return 'Browsey could not start the selected application'
    case 'permission_denied':
      return 'Permission denied'
    case 'invalid_path':
    case 'path_not_absolute':
      return 'This item could not be opened from its current path'
    case 'database_open_failed':
      return 'Browsey could not prepare the application launch'
    default:
      return message
  }
}

const invokeOpenWith = async <T>(command: string, args: Record<string, unknown>) => {
  try {
    return await invoke<T>(command, args)
  } catch (err) {
    const normalized = normalizeError(err)
    normalized.message = userOpenWithErrorMessage(normalized.code, normalized.message)
    throw normalized
  }
}

export const fetchOpenWithApps = (path: string) =>
  invokeOpenWith<OpenWithApp[]>('list_open_with_apps', { path })

export const openWithSelection = (path: string, choice: OpenWithChoice) =>
  invokeOpenWith<void>('open_with', { path, choice })

export const defaultOpenWithApp: OpenWithApp = {
  id: '__default__',
  name: 'Open normally',
  comment: 'Use the system default handler',
  exec: 'open default',
  icon: undefined,
  matches: true,
  terminal: false,
}

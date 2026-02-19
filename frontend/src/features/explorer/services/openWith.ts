import { invoke } from '@/lib/tauri'

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

export const fetchOpenWithApps = (path: string) =>
  invoke<OpenWithApp[]>('list_open_with_apps', { path })

export const openWithSelection = (path: string, choice: OpenWithChoice) =>
  invoke<void>('open_with', { path, choice })

export const defaultOpenWithApp: OpenWithApp = {
  id: '__default__',
  name: 'Open normally',
  comment: 'Use the system default handler',
  exec: 'open default',
  icon: undefined,
  matches: true,
  terminal: false,
}

import { invoke } from '@/shared/lib/tauri'
import type { Entry, Partition } from '../explorer/types'

export type NetworkUriKind = 'not_uri' | 'mountable' | 'external' | 'unsupported'

export type NetworkUriClassification = {
  kind: NetworkUriKind
  scheme: string | null
  normalizedUri: string | null
}

export type ConnectNetworkUriResult = {
  kind: NetworkUriKind
  normalizedUri: string | null
  mountedPath: string | null
}

export const listNetworkDevices = () =>
  invoke<Partition[]>('list_network_devices')

export const listNetworkEntries = (forceRefresh = false) =>
  invoke<Entry[]>('list_network_entries', { force_refresh: forceRefresh })

export const openNetworkUri = (uri: string) =>
  invoke<void>('open_network_uri', { uri })

export const classifyNetworkUri = (uri: string) =>
  invoke<NetworkUriClassification>('classify_network_uri', { uri })

export const resolveMountedPathForUri = (uri: string) =>
  invoke<string | null>('resolve_mounted_path_for_uri', { uri })

export const connectNetworkUri = (uri: string) =>
  invoke<ConnectNetworkUriResult>('connect_network_uri', { uri })

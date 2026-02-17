import { invoke } from '@tauri-apps/api/core'
import type { Entry, Partition } from '../explorer/types'

export type NetworkUriKind = 'not_uri' | 'mountable' | 'external' | 'unsupported'

export type NetworkUriClassification = {
  kind: NetworkUriKind
  scheme: string | null
  normalizedUri: string | null
}

export const listNetworkDevices = () =>
  invoke<Partition[]>('list_network_devices')

export const listNetworkEntries = () =>
  invoke<Entry[]>('list_network_entries')

export const openNetworkUri = (uri: string) =>
  invoke<void>('open_network_uri', { uri })

export const classifyNetworkUri = (uri: string) =>
  invoke<NetworkUriClassification>('classify_network_uri', { uri })

export const resolveMountedPathForUri = (uri: string) =>
  invoke<string | null>('resolve_mounted_path_for_uri', { uri })

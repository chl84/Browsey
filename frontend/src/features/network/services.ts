import { invoke } from '@tauri-apps/api/core'
import type { Partition } from '../explorer/types'

export const listNetworkDevices = () =>
  invoke<Partition[]>('list_network_devices')

export const openNetworkUri = (uri: string) =>
  invoke<void>('open_network_uri', { uri })

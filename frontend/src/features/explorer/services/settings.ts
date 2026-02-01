import { invoke } from '@tauri-apps/api/core'

export const loadShowHidden = () => invoke<boolean | null>('load_show_hidden')

export const storeShowHidden = (value: boolean) =>
  invoke<void>('store_show_hidden', { value })

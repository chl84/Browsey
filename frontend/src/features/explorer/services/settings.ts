import { invoke } from '@tauri-apps/api/core'

export const loadShowHidden = () => invoke<boolean | null>('load_show_hidden')

export const storeShowHidden = (value: boolean) =>
  invoke<void>('store_show_hidden', { value })

export const loadHiddenFilesLast = () => invoke<boolean | null>('load_hidden_files_last')

export const storeHiddenFilesLast = (value: boolean) =>
  invoke<void>('store_hidden_files_last', { value })

import { invoke } from '@tauri-apps/api/core'

export const loadShowHidden = () => invoke<boolean | null>('load_show_hidden')

export const storeShowHidden = (value: boolean) =>
  invoke<void>('store_show_hidden', { value })

export const loadHiddenFilesLast = () => invoke<boolean | null>('load_hidden_files_last')

export const storeHiddenFilesLast = (value: boolean) =>
  invoke<void>('store_hidden_files_last', { value })

export const loadFoldersFirst = () => invoke<boolean | null>('load_folders_first')

export const storeFoldersFirst = (value: boolean) =>
  invoke<void>('store_folders_first', { value })

export const loadDefaultView = () => invoke<'list' | 'grid' | null>('load_default_view')

export const storeDefaultView = (value: 'list' | 'grid') =>
  invoke<void>('store_default_view', { value })

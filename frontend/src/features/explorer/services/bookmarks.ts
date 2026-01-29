import { invoke } from '@tauri-apps/api/core'

export const addBookmark = (label: string, path: string) =>
  invoke<void>('add_bookmark', { label, path })

export const removeBookmark = (path: string) =>
  invoke<void>('remove_bookmark', { path })

export const getBookmarks = () =>
  invoke<{ label: string; path: string }[]>('get_bookmarks')

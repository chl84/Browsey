import { invoke } from '@/lib/tauri'

export type ThumbnailCacheClearResult = {
  removed_files: number
  removed_bytes: number
}

export const clearStars = () => invoke<number>('clear_stars')

export const clearBookmarks = () => invoke<number>('clear_bookmarks')

export const clearRecents = () => invoke<number>('clear_recents')

export const clearThumbnailCache = () =>
  invoke<ThumbnailCacheClearResult>('clear_thumbnail_cache')

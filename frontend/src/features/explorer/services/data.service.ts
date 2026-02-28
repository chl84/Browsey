import { invoke } from '@/shared/lib/tauri'

export type CacheClearResult = {
  removed_files: number
  removed_bytes: number
}

export const clearStars = () => invoke<number>('clear_stars')

export const clearBookmarks = () => invoke<number>('clear_bookmarks')

export const clearRecents = () => invoke<number>('clear_recents')

export const clearThumbnailCache = () =>
  invoke<CacheClearResult>('clear_thumbnail_cache')

export const clearCloudOpenCache = () =>
  invoke<CacheClearResult>('clear_cloud_open_cache')

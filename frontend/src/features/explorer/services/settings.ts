import { invoke } from '@tauri-apps/api/core'
import type { DefaultSortField, Density } from '../types'

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

export const loadStartDir = () => invoke<string | null>('load_start_dir')

export const storeStartDir = (value: string) =>
  invoke<void>('store_start_dir', { value })

export const loadConfirmDelete = () => invoke<boolean | null>('load_confirm_delete')

export const storeConfirmDelete = (value: boolean) =>
  invoke<void>('store_confirm_delete', { value })

export const loadSortField = () => invoke<DefaultSortField | null>('load_sort_field')

export const storeSortField = (value: DefaultSortField) =>
  invoke<void>('store_sort_field', { value })

export const loadSortDirection = () => invoke<'asc' | 'desc' | null>('load_sort_direction')

export const storeSortDirection = (value: 'asc' | 'desc') =>
  invoke<void>('store_sort_direction', { value })

export const loadDensity = () => invoke<Density | null>('load_density')

export const storeDensity = (value: Density) => invoke<void>('store_density', { value })

export const loadArchiveName = () => invoke<string | null>('load_archive_name')

export const storeArchiveName = (value: string) => invoke<void>('store_archive_name', { value })

export const loadArchiveLevel = () => invoke<number | null>('load_archive_level')

export const storeArchiveLevel = (value: number) =>
  invoke<void>('store_archive_level', { value })

export const loadOpenDestAfterExtract = () => invoke<boolean | null>('load_open_dest_after_extract')

export const storeOpenDestAfterExtract = (value: boolean) =>
  invoke<void>('store_open_dest_after_extract', { value })

export const loadFfmpegPath = () => invoke<string | null>('load_ffmpeg_path')

export const storeFfmpegPath = (value: string) =>
  invoke<void>('store_ffmpeg_path', { value })

export const loadVideoThumbs = () => invoke<boolean | null>('load_video_thumbs')

export const storeVideoThumbs = (value: boolean) =>
  invoke<void>('store_video_thumbs', { value })

export const loadThumbCacheMb = () => invoke<number | null>('load_thumb_cache_mb')

export const storeThumbCacheMb = (value: number) =>
  invoke<void>('store_thumb_cache_mb', { value })

export const loadMountsPollMs = () => invoke<number | null>('load_mounts_poll_ms')

export const storeMountsPollMs = (value: number) =>
  invoke<void>('store_mounts_poll_ms', { value })

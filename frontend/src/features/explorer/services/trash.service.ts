import { invoke } from '@/shared/lib/tauri'

export const deleteEntries = (paths: string[], progressEvent?: string) =>
  invoke<void>('delete_entries', { paths, progressEvent })

export const moveToTrashMany = (paths: string[], progressEvent?: string) =>
  invoke<void>('move_to_trash_many', { paths, progressEvent })

export const purgeTrashItems = (ids: string[]) =>
  invoke<void>('purge_trash_items', { ids })

export const restoreTrashItems = (ids: string[]) =>
  invoke<void>('restore_trash_items', { ids })

export const removeRecent = (paths: string[]) =>
  invoke<void>('remove_recent', { paths })

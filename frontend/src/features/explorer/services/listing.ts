import { invoke } from '@tauri-apps/api/core'
import type { Listing, ListingFacets, Partition, SortField, SortDirection } from '../types'

export type FacetScope = 'dir' | 'recent' | 'starred' | 'trash'

export const listDir = (path: string | undefined, sort: { field: SortField; direction: SortDirection }) =>
  invoke<Listing>('list_dir', { path, sort })

export const listRecent = (sort: { field: SortField; direction: SortDirection } | null) =>
  invoke<Listing>('list_recent', { sort })

export const listStarred = (sort: { field: SortField; direction: SortDirection } | null) =>
  invoke<Listing>('list_starred', { sort })

export const listTrash = (sort: { field: SortField; direction: SortDirection }) =>
  invoke<Listing>('list_trash', { sort })

export const listFacets = (args: {
  scope: FacetScope
  path?: string
  includeHidden?: boolean
}) => invoke<ListingFacets>('list_facets', args)

export const watchDir = (path: string) =>
  invoke<void>('watch_dir', { path })

export const listMounts = () =>
  invoke<Partition[]>('list_mounts')

export const searchStream = (args: {
  path: string
  query: string
  sort: { field: SortField; direction: SortDirection }
  progressEvent: string
}) => invoke<void>('search_stream', args)

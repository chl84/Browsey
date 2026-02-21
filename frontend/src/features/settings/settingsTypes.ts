import type { DefaultSortField, Density } from '@/features/explorer'

export type SortField = DefaultSortField
export type SortDirection = 'asc' | 'desc'
export type LogLevel = 'error' | 'warn' | 'info' | 'debug'
export type DataClearTarget = 'thumb-cache' | 'stars' | 'bookmarks' | 'recents'

export type Settings = {
  startDir: string
  defaultView: 'list' | 'grid'
  foldersFirst: boolean
  hiddenFilesLast: boolean
  showHidden: boolean
  confirmDelete: boolean
  sortField: SortField
  sortDirection: SortDirection
  density: Density
  iconSize: number
  archiveName: string
  archiveLevel: number
  openDestAfterExtract: boolean
  videoThumbs: boolean
  hardwareAcceleration: boolean
  ffmpegPath: string
  thumbCacheMb: number
  mountsPollMs: number
  doubleClickMs: number
  logLevel: LogLevel
  externalTools: string
  highContrast: boolean
  scrollbarWidth: number
}

export const DEFAULT_SETTINGS: Settings = {
  startDir: '~',
  defaultView: 'list',
  foldersFirst: true,
  hiddenFilesLast: false,
  showHidden: true,
  confirmDelete: true,
  sortField: 'name',
  sortDirection: 'asc',
  density: 'cozy',
  iconSize: 24,
  archiveName: 'Archive',
  archiveLevel: 6,
  openDestAfterExtract: false,
  videoThumbs: true,
  hardwareAcceleration: true,
  ffmpegPath: '',
  thumbCacheMb: 300,
  mountsPollMs: 8000,
  doubleClickMs: 300,
  logLevel: 'warn',
  externalTools: '',
  highContrast: false,
  scrollbarWidth: 10,
}

export const clearTargetCopy = {
  'thumb-cache': {
    title: 'Clear thumbnail cache?',
    message: 'This removes all cached thumbnail files on disk and refreshes the UI.',
    confirmLabel: 'Clear cache',
  },
  stars: {
    title: 'Clear all stars?',
    message: 'This removes all starred items.',
    confirmLabel: 'Clear stars',
  },
  bookmarks: {
    title: 'Clear all bookmarks?',
    message: 'This removes all bookmarks.',
    confirmLabel: 'Clear bookmarks',
  },
  recents: {
    title: 'Clear all recents?',
    message: 'This removes all recent items.',
    confirmLabel: 'Clear recents',
  },
} satisfies Record<DataClearTarget, { title: string; message: string; confirmLabel: string }>

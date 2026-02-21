type ExplorerEntry = {
  name: string
  path: string
  kind: 'dir' | 'file' | 'link'
  ext?: string | null
  size?: number | null
  items?: number | null
  modified?: string | null
  iconId: number
  starred?: boolean
  hidden?: boolean
}

type Listing = {
  current: string
  entries: ExplorerEntry[]
}

const ROOT = '/mock'

const FILE_TREE: Record<string, ExplorerEntry[]> = {
  [ROOT]: [
    {
      name: 'Documents',
      path: '/mock/Documents',
      kind: 'dir',
      items: 2,
      modified: '2026-02-21 10:00',
      iconId: 4,
    },
    {
      name: 'notes.txt',
      path: '/mock/notes.txt',
      kind: 'file',
      ext: 'txt',
      size: 1280,
      modified: '2026-02-20 09:15',
      iconId: 12,
    },
  ],
  '/mock/Documents': [
    {
      name: 'report.txt',
      path: '/mock/Documents/report.txt',
      kind: 'file',
      ext: 'txt',
      size: 4096,
      modified: '2026-02-21 11:30',
      iconId: 12,
    },
    {
      name: 'archive',
      path: '/mock/Documents/archive',
      kind: 'dir',
      items: 0,
      modified: '2026-02-19 14:05',
      iconId: 4,
    },
  ],
}

const cloneEntries = (entries: ExplorerEntry[]) => entries.map((entry) => ({ ...entry }))

const listDirMock = (path?: string | null): Listing => {
  const current = typeof path === 'string' && path.length > 0 ? path : ROOT
  const entries = FILE_TREE[current] ?? []
  return {
    current,
    entries: cloneEntries(entries),
  }
}

const emptyFacets = {
  name: [],
  type: [],
  modified: [],
  size: [],
}

export const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  switch (cmd) {
    case 'list_dir':
      return listDirMock(args?.path as string | undefined) as T
    case 'list_recent':
      return { current: 'recent://', entries: [] } as T
    case 'list_starred':
      return { current: 'starred://', entries: [] } as T
    case 'list_trash':
      return { current: 'trash://', entries: [] } as T
    case 'list_facets':
      return emptyFacets as T
    case 'list_mounts':
      return [] as T
    case 'watch_dir':
      return undefined as T
    case 'get_bookmarks':
      return [] as T
    case 'load_saved_column_widths':
      return null as T
    case 'load_shortcuts':
      return [] as T
    case 'set_shortcut_binding':
      return [] as T
    case 'reset_shortcut_binding':
      return [] as T
    case 'reset_all_shortcuts':
      return [] as T
    case 'load_default_view':
      return 'list' as T
    case 'load_show_hidden':
      return false as T
    case 'load_hidden_files_last':
      return false as T
    case 'load_folders_first':
      return true as T
    case 'load_start_dir':
      return ROOT as T
    case 'load_confirm_delete':
      return true as T
    case 'load_sort_field':
      return 'name' as T
    case 'load_sort_direction':
      return 'asc' as T
    case 'load_density':
      return 'cozy' as T
    case 'load_archive_name':
      return 'Archive' as T
    case 'load_archive_level':
      return 6 as T
    case 'load_open_dest_after_extract':
      return true as T
    case 'load_video_thumbs':
      return true as T
    case 'load_hardware_acceleration':
      return true as T
    case 'load_ffmpeg_path':
      return '' as T
    case 'load_thumb_cache_mb':
      return 300 as T
    case 'load_mounts_poll_ms':
      return 8000 as T
    case 'load_double_click_ms':
      return 300 as T
    case 'system_clipboard_paths':
      return { mode: 'copy', paths: [] } as T
    case 'can_extract_paths':
      return false as T
    case 'open_entry':
      return undefined as T
    default:
      return null as T
  }
}

export const convertFileSrc = (path: string) => path

export class Channel<T = unknown> {
  onmessage: ((message: T) => void) | null = null
}

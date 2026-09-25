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

type SearchProgressPayload = {
  entries: ExplorerEntry[]
  done: boolean
  error?: string
}

type ClipboardMode = 'copy' | 'cut'

type MockClipboardState = {
  mode: ClipboardMode
  paths: string[]
}

type E2eMockControl = {
  systemClipboard?: MockClipboardState
  failCommands?: string[]
  formatHold?: boolean
  formatProgress?: { phase: string; percent: number | null }
  formatError?: { code: string; message: string }
  calls?: Array<{ cmd: string; args?: Record<string, unknown> }>
  partitions?: Array<{ label: string; path: string; fs?: string; removable?: boolean }>
}

import { emitMockEvent } from './event'

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

let internalClipboard: MockClipboardState = { mode: 'copy', paths: [] }

const basename = (path: string) => {
  const idx = path.lastIndexOf('/')
  return idx >= 0 ? path.slice(idx + 1) : path
}

const joinPath = (dir: string, name: string) => `${dir.replace(/\/+$/, '')}/${name}`

const renameCandidate = (baseName: string, attempt: number) => {
  if (attempt === 0) return baseName
  const dot = baseName.lastIndexOf('.')
  const hasExt = dot > 0
  const stem = hasExt ? baseName.slice(0, dot) : baseName
  const ext = hasExt ? baseName.slice(dot) : ''
  return `${stem}-${attempt}${ext}`
}

const findEntry = (path: string): ExplorerEntry | null => {
  for (const entries of Object.values(FILE_TREE)) {
    const found = entries.find((entry) => entry.path === path)
    if (found) return found
  }
  return null
}

const removeEntry = (path: string) => {
  for (const [dir, entries] of Object.entries(FILE_TREE)) {
    const next = entries.filter((entry) => entry.path !== path)
    if (next.length !== entries.length) {
      FILE_TREE[dir] = next
      return
    }
  }
}

const ensureDirListing = (path: string) => {
  if (!FILE_TREE[path]) {
    FILE_TREE[path] = []
  }
}

const copyOrMoveFromClipboard = (
  dest: string,
  policy: 'rename' | 'overwrite' = 'rename',
) => {
  ensureDirListing(dest)
  const destEntries = FILE_TREE[dest]
  for (const sourcePath of internalClipboard.paths) {
    const source = findEntry(sourcePath)
    if (!source) continue
    const baseName = basename(source.path)
    let finalName = baseName
    if (policy === 'rename') {
      let attempt = 0
      while (destEntries.some((entry) => entry.name === finalName)) {
        attempt += 1
        finalName = renameCandidate(baseName, attempt)
      }
    }
    const targetPath = joinPath(dest, finalName)
    if (policy === 'overwrite') {
      const filtered = destEntries.filter((entry) => entry.name !== finalName)
      FILE_TREE[dest] = filtered
    }
    FILE_TREE[dest].push({
      ...source,
      name: finalName,
      path: targetPath,
    })

    if (internalClipboard.mode === 'cut') {
      removeEntry(source.path)
    }
  }
}

const e2eControl = (): E2eMockControl | null => {
  const fromGlobal = (globalThis as { __BROWSEY_E2E__?: unknown }).__BROWSEY_E2E__
  if (!fromGlobal || typeof fromGlobal !== 'object') {
    return null
  }
  return fromGlobal as E2eMockControl
}

const shouldFailCommand = (cmd: string) => e2eControl()?.failCommands?.includes(cmd) === true

const listDirMock = (path?: string | null): Listing => {
  const current = typeof path === 'string' && path.length > 0 ? path : ROOT
  const entries = FILE_TREE[current] ?? []
  return {
    current,
    entries: cloneEntries(entries),
  }
}

const flattenTreeUnder = (root: string): ExplorerEntry[] => {
  const prefix = `${root.replace(/\/+$/, '')}/`
  return Object.entries(FILE_TREE).flatMap(([dir, entries]) => {
    if (dir !== root && !dir.startsWith(prefix)) {
      return []
    }
    return entries
  })
}

const searchEntriesMock = (path: string, query: string): ExplorerEntry[] => {
  const needle = query.trim().toLowerCase()
  if (needle.length === 0) return []
  return flattenTreeUnder(path)
    .filter((entry) => entry.name.toLowerCase().includes(needle))
    .map((entry) => ({ ...entry }))
}

const emptyFacets = {
  name: [],
  type: [],
  modified: [],
  size: [],
}

export const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  e2eControl()?.calls?.push({ cmd, args })
  if (shouldFailCommand(cmd)) {
    throw new Error(`Simulated ${cmd} failure`)
  }

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
    case 'context_menu_actions': {
      const count = Number(args?.count ?? 0)
      if (count > 1) {
        return [{ id: 'rename-advanced', label: 'Rename…' }] as T
      }
      if (count === 1) {
        return [
          { id: 'rename', label: 'Rename…' },
          { id: 'open-with', label: 'Open with…' },
          { id: 'compress', label: 'Compress…' },
        ] as T
      }
      return [] as T
    }
    case 'list_mounts':
      return (e2eControl()?.partitions ?? []) as T
    case 'watch_dir':
      return undefined as T
    case 'search_stream': {
      const path = typeof args?.path === 'string' ? args.path : ROOT
      const query = typeof args?.query === 'string' ? args.query : ''
      const progressEvent =
        typeof args?.progressEvent === 'string' ? args.progressEvent : 'search-progress-mock'
      const entries = searchEntriesMock(path, query)
      queueMicrotask(() => {
        emitMockEvent<SearchProgressPayload>(progressEvent, {
          entries,
          done: true,
        })
      })
      return undefined as T
    }
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
    case 'load_theme_mode':
      return 'dark' as T
    case 'load_system_theme':
      return {
        name: 'Nord',
        mode: 'dark',
        accent: '#81a1c1',
        selection: '#434c5e',
        muted: '#4c566a',
        background: '#2e3440',
        darkBackground: '#222730',
        darkerBackground: '#191c23',
        lighterBackground: '#3b4252',
        foreground: '#d8dee9',
        darkForeground: '#667080',
        lightForeground: '#adb5c4',
        brightForeground: '#d8dee9',
        red: '#bf616a',
        yellow: '#ebcb8b',
        orange: '#d5967a',
        green: '#a3be8c',
        cyan: '#88c0d0',
        blue: '#81a1c1',
        magenta: '#b48ead',
      } as T
    case 'store_theme_mode':
    case 'load_archive_name':
      return 'Archive' as T
    case 'load_archive_level':
      return 6 as T
    case 'load_open_dest_after_extract':
      return true as T
    case 'load_video_thumbs':
      return true as T
    case 'load_cloud_thumbs':
      return false as T
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
      return (e2eControl()?.systemClipboard ?? { mode: 'copy', paths: [] }) as T
    case 'set_clipboard_cmd':
      internalClipboard = {
        mode: (args?.mode as ClipboardMode) ?? 'copy',
        paths: Array.isArray(args?.paths) ? (args?.paths as string[]) : [],
      }
      return undefined as T
    case 'copy_paths_to_system_clipboard':
    case 'clear_system_clipboard':
      return undefined as T
    case 'paste_clipboard_preview':
      return [] as T
    case 'paste_clipboard_cmd':
      copyOrMoveFromClipboard(
        (args?.dest as string) ?? ROOT,
        ((args?.policy as 'rename' | 'overwrite' | undefined) ?? 'rename'),
      )
      return undefined as T
    case 'can_extract_paths':
      return false as T
    case 'get_removable_usb_format_info':
      return {
        device: '/dev/sdz',
        model: 'Mock USB drive',
        sizeBytes: 32000000000,
        filesystems: [
          { id: 'exfat', label: 'exFAT', description: 'Compatible everywhere', available: true },
          { id: 'fat32', label: 'FAT32', description: '4 GB file limit', available: true },
          { id: 'ext4', label: 'ext4', description: 'Linux filesystem', available: true },
          { id: 'btrfs', label: 'Btrfs', description: 'Linux filesystem', available: true },
        ],
      } as T
    case 'mount_usb_volume': {
      const control = e2eControl()
      if (control?.partitions) {
        control.partitions = control.partitions.map((part) => part.path === args?.path ? { ...part, path: '/mock/USB' } : part)
      }
      emitMockEvent('volumes-changed', null)
      return '/mock/USB' as T
    }
    case 'compress_entries':
      return `/mock/${args?.name}` as T
    case 'list_open_with_apps':
      return [
        { id: 'alpha', name: 'Alpha editor', exec: 'alpha', matches: true, terminal: false },
        { id: 'beta', name: 'Beta editor', exec: 'beta', matches: false, terminal: false },
      ] as T
    case 'open_with':
      return undefined as T
    case 'format_removable_partition': {
      const control = e2eControl()
      const channel = args?.onProgress as Channel<{ phase: string; percent: number | null }> | undefined
      while (control?.formatHold) {
        channel?.onmessage?.(control.formatProgress ?? { phase: 'Creating partition and filesystem', percent: null })
        await new Promise((resolve) => setTimeout(resolve, 100))
      }
      if (control?.formatError) throw control.formatError
      return {
        device: '/dev/sdz',
        mountPath: '/mock/USB',
        sizeBytes: 32000000000,
        filesystem: args?.filesystem ?? 'exFAT',
        label: args?.label || null,
      } as T
    }
    case 'open_entry':
    case 'open_cloud_entry':
      return undefined as T
    default:
      return null as T
  }
}

export const convertFileSrc = (path: string) => path

export class Channel<T = unknown> {
  onmessage: ((message: T) => void) | null = null
}

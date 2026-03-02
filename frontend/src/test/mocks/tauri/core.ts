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

type ClipboardMode = 'copy' | 'cut'

type MockClipboardState = {
  mode: ClipboardMode
  paths: string[]
}

type E2eMockControl = {
  systemClipboard?: MockClipboardState
  failCommands?: string[]
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

let internalClipboard: MockClipboardState = { mode: 'copy', paths: [] }

const basename = (path: string) => {
  const idx = path.lastIndexOf('/')
  return idx >= 0 ? path.slice(idx + 1) : path
}

const dirname = (path: string) => {
  const idx = path.lastIndexOf('/')
  return idx > 0 ? path.slice(0, idx) : ROOT
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

const emptyFacets = {
  name: [],
  type: [],
  modified: [],
  size: [],
}

export const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
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

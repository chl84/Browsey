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
  network?: boolean
}

type Listing = {
  current: string
  entries: ExplorerEntry[]
  pendingMetadataPaths?: string[]
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

type MockCloudCopy = { id: string; sourcePath: string; localPath: string; originalSize: number | null; originalModified: string | null; originalHash: string; createdAt: number; dirty: boolean; uploadedPath: string | null; saveStatus?: string; storageBytes?: number; cleanupBlockedReason?: string | null }
type E2eMockControl = {
  cloudFixture?: boolean
  cloudCopies?: MockCloudCopy[]
  cloudStatuses?: unknown[]
  cloudSaveError?: { code: string; message: string }
  cloudUploadChanged?: boolean
  windowControlPolicy?: { minimize: boolean; maximize: boolean }
  bookmarks?: Array<{ label: string; path: string }>
  thumbnailFixture?: boolean
  performanceFixture?: { entries: number; thumbnailDelayMs?: number }
  defaultView?: 'list' | 'grid'
  startupPath?: string | null
  thumbnailHold?: boolean
  systemClipboard?: MockClipboardState
  failCommands?: string[]
  columnWidths?: number[]
  listingEntries?: ExplorerEntry[]
  listingSnapshot?: Listing
  listingHold?: boolean
  sortField?: 'name' | 'type' | 'modified' | 'size'
  trashEntries?: ExplorerEntry[]
  emptyTrashHold?: boolean
  undoStorage?: { directory: string; exists: boolean; sessions: number; markedSessions: number; files: number; logicalBytes: number; incomplete: boolean }
  undoStorageHold?: boolean
  archivePassword?: string
  formatHold?: boolean
  ntfsFormatAvailable?: boolean
  formatProgress?: { phase: string; percent: number | null }
  formatError?: { code: string; message: string }
  mtpHold?: boolean
  mtpError?: string
  networkConnections?: Array<{ uri: string; label: string }>
  networkMountedPaths?: Record<string, string>
  networkConnectError?: string
  networkTrashSupported?: boolean
  trashSupported?: boolean
  networkDeleteHold?: boolean
  selectionActionsFixture?: boolean
  calls?: Array<{ cmd: string; args?: Record<string, unknown> }>
  partitions?: Array<{ label: string; path: string; fs?: string; removable?: boolean; sizeBytes?: number | null }>
  volumeUsage?: { totalBytes: number; usedBytes: number; freeBytes: number; reservedBytes: number } | null
  permissionRestriction?: 'write_protection' | 'mount_managed' | 'read_only' | 'network_managed' | null
}

import { emitMockEvent } from './event'

const ROOT = '/mock'
const cancelledThumbnails = new Set<string>()

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
  input: MockClipboardState = internalClipboard,
) => {
  ensureDirListing(dest)
  const destEntries = FILE_TREE[dest]
  for (const sourcePath of input.paths) {
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

    if (input.mode === 'cut') {
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
  // Match the backend's Home expansion rather than treating ~ as a directory.
  const current = typeof path === 'string' && path.length > 0 && path !== '~' ? path : ROOT
  const entries = (current === ROOT ? e2eControl()?.listingEntries : undefined) ?? FILE_TREE[current] ?? []
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
  const control = e2eControl()
  control?.calls?.push({ cmd, args })
  if (shouldFailCommand(cmd)) {
    throw new Error(`Simulated ${cmd} failure`)
  }

  switch (cmd) {
    case 'get_window_control_policy':
      return (control?.windowControlPolicy ?? { minimize: true, maximize: true }) as T
    case 'get_startup_path':
      return (control?.startupPath ?? null) as T
    case 'list_dir':
      while (control?.listingHold) await new Promise(resolve => setTimeout(resolve, 20))
      if (control?.listingSnapshot && control.listingSnapshot.current === args?.path) {
        return { ...control.listingSnapshot, entries: cloneEntries(control.listingSnapshot.entries) } as T
      }
      if (control?.cloudFixture && String(args?.path).startsWith('rclone://')) {
        const current = String(args?.path)
        return { current, entries: [{ name: 'report.txt', path: `${current}/report.txt`, kind: 'file', size: 8, iconId: 12,
          capabilities: { canList: true, canMkdir: true, canDelete: true, canRename: true, canMove: true, canCopy: true,
            canTrash: true, canUndo: false, canPermissions: false, canCreateFile: true, canOpenWith: true,
            canArchive: true, canAdvancedRename: true, canExternalCopy: true } }] } as T
      }
      if (control?.thumbnailFixture) {
        const current = (args?.path as string | undefined) || ROOT
        const requested = control.performanceFixture?.entries ?? 100
        const count = Number.isSafeInteger(requested) ? Math.max(1, Math.min(100_000, requested)) : 100
        const entries: ExplorerEntry[] = Array.from({ length: current === ROOT ? count : 1 }, (_, i) => ({
          name: `photo-${i.toString().padStart(3, '0')}.jpg`,
          path: `${current}/photo-${i.toString().padStart(3, '0')}.jpg`,
          kind: 'file', size: 4096, modified: '2026-09-25 12:00', iconId: 12,
        }))
        if (current === ROOT) entries.unshift(...cloneEntries(FILE_TREE[ROOT]))
        return { current, entries } as T
      }
      return listDirMock(args?.path as string | undefined) as T
    case 'cancel_task':
      cancelledThumbnails.add(String(args?.id))
      return undefined as T
    case 'get_thumbnail': {
      const id = String(args?.requestId)
      const delay = control?.performanceFixture?.thumbnailDelayMs ?? 0
      const deadline = Date.now() + Math.max(0, Math.min(5000, delay))
      while (Date.now() < deadline && !cancelledThumbnails.has(id)) {
        await new Promise(resolve => setTimeout(resolve, 20))
      }
      while (control?.thumbnailHold && !cancelledThumbnails.has(id)) {
        await new Promise(resolve => setTimeout(resolve, 20))
      }
      if (cancelledThumbnails.delete(id)) throw new Error('Thumbnail cancelled')
      const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96" fill="green"/></svg>'
      return { path: `data:image/svg+xml,${encodeURIComponent(svg)}`, width: 96, height: 96, cached: false } as T
    }
    case 'list_recent':
      return { current: 'recent://', entries: [] } as T
    case 'list_starred':
      return { current: 'starred://', entries: [] } as T
    case 'list_trash':
      return { current: 'trash://', entries: e2eControl()?.trashEntries ?? [] } as T
    case 'empty_trash': {
      const control = e2eControl()
      while (control?.emptyTrashHold) await new Promise(resolve => setTimeout(resolve, 20))
      if (control) control.trashEntries = []
      return undefined as T
    }
    case 'list_facets':
      return emptyFacets as T
    case 'context_menu_actions': {
      const count = Number(args?.count ?? 0)
      if (count > 0 && control?.selectionActionsFixture) {
        return [
          { id: 'copy', label: 'Copy' }, { id: 'cut', label: 'Cut' },
          { id: 'move-trash', label: 'Move to wastebasket' },
          { id: 'delete-permanent', label: 'Delete permanently…' },
        ] as T
      }
      if (count > 1) {
        return [{ id: 'rename-advanced', label: 'Rename…' }] as T
      }
      if (count === 1) {
        return [
          { id: 'rename', label: 'Rename…' },
          { id: 'open-with', label: 'Open with…' },
          { id: 'compress', label: 'Compress…' },
          ...(control?.archivePassword !== undefined ? [{ id: 'extract', label: 'Extract' }] : []),
          { id: 'properties', label: 'Properties' },
        ] as T
      }
      return [] as T
    }
    case 'list_mounts':
      return (e2eControl()?.partitions ?? []) as T
    case 'get_volume_usage':
      return (e2eControl()?.volumeUsage ?? null) as T
    case 'watch_dir':
      return undefined as T
    case 'inspect_undo_storage': {
      while (control?.undoStorageHold) await new Promise(resolve => setTimeout(resolve, 20))
      return (control?.undoStorage ?? {
        directory: '/mock/browsey/undo-sessions', exists: false, sessions: 0,
        markedSessions: 0, files: 0, logicalBytes: 0, incomplete: false,
      }) as T
    }
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
      return (control?.bookmarks ?? []) as T
    case 'load_saved_column_widths':
      return (control?.columnWidths ?? null) as T
    case 'load_shortcuts':
      return [] as T
    case 'set_shortcut_binding':
      return [] as T
    case 'reset_shortcut_binding':
      return [] as T
    case 'reset_all_shortcuts':
      return [] as T
    case 'load_default_view':
      return (control?.defaultView ?? (control?.thumbnailFixture ? 'grid' : 'list')) as T
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
      return (control?.sortField ?? 'name') as T
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
    case 'paste_clipboard_preview': {
      const input = (args?.input as MockClipboardState | undefined) ?? internalClipboard
      const dest = (args?.dest as string) ?? ROOT
      return input.paths.flatMap(src => {
        const target = joinPath(dest, basename(src))
        const existing = findEntry(target)
        return existing ? [{ src, target, exists: true, is_dir: existing.kind === 'dir' }] : []
      }) as T
    }
    case 'paste_clipboard_cmd':
      copyOrMoveFromClipboard(
        (args?.dest as string) ?? ROOT,
        ((args?.policy as 'rename' | 'overwrite' | undefined) ?? 'rename'),
        args?.input as MockClipboardState | undefined,
      )
      return undefined as T
    case 'can_extract_paths':
      return (control?.archivePassword !== undefined) as T
    case 'get_permissions':
      return {
        restriction: control?.permissionRestriction ?? null,
        access_supported: true,
        executable_supported: !control?.permissionRestriction,
        ownership_supported: !control?.permissionRestriction,
        read_only: false, executable: true, owner_name: 'chris', group_name: 'users',
        owner: { read: true, write: true, exec: true },
        group: { read: true, write: false, exec: true },
        other: { read: true, write: false, exec: true },
      } as T
    case 'entry_times_cmd':
      return { accessed: null, modified: null, created: null } as T
    case 'list_ownership_principals':
      return (args?.kind === 'user' ? ['chris', 'root'] : ['users', 'root']) as T
    case 'get_removable_usb_format_info':
      return {
        device: '/dev/sdz',
        model: 'Mock USB drive',
        sizeBytes: 32000000000,
        filesystems: [
          { id: 'exfat', label: 'exFAT', description: 'Compatible everywhere', available: true, requiredTool: 'mkfs.exfat', labelMaxLength: 11 },
          { id: 'fat32', label: 'FAT32', description: '4 GB file limit', available: true, requiredTool: 'mkfs.fat', labelMaxLength: 11 },
          { id: 'ext4', label: 'ext4', description: 'Linux filesystem', available: true, requiredTool: 'mkfs.ext4', labelMaxLength: 11 },
          { id: 'btrfs', label: 'Btrfs', description: 'Linux filesystem', available: true, requiredTool: 'mkfs.btrfs', labelMaxLength: 11 },
          { id: 'ntfs', label: 'NTFS', description: 'Windows filesystem; also usable on Linux', available: control?.ntfsFormatAvailable ?? true, requiredTool: 'mkntfs', labelMaxLength: 128 },
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
    case 'connect_network_uri': {
      const control = e2eControl()
      if (typeof args?.uri === 'string' && !args.uri.startsWith('mtp://')) {
        if (control?.networkConnectError) throw { code: 'mount_failed', message: control.networkConnectError }
        const uri = args.uri
        const mountedPath = control?.networkMountedPaths?.[uri] ?? '/mock/Network'
        if (control) {
          control.networkMountedPaths ??= {}
          control.networkMountedPaths[uri] = mountedPath
          control.networkConnections ??= []
          if (!control.networkConnections.some(connection => connection.uri === uri)) {
            control.networkConnections.push({ uri, label: `SFTP (${new URL(uri).host})` })
          }
        }
        return { kind: 'mountable', normalizedUri: uri, mountedPath } as T
      }
      while (control?.mtpHold) await new Promise((resolve) => setTimeout(resolve, 50))
      if (control?.mtpError) throw { code: 'mount_failed', message: control.mtpError }
      if (!control?.partitions?.some((part) => part.path === args?.uri)) {
        throw { code: 'mount_failed', message: 'Phone disconnected or unavailable.' }
      }
      const mountedPath = '/mock/Phone'
      control.partitions = control.partitions.map((part) => part.path === args?.uri ? { ...part, path: mountedPath } : part)
      emitMockEvent('volumes-changed', null)
      return { kind: 'mountable', normalizedUri: args?.uri, mountedPath } as T
    }
    case 'list_saved_network_connections':
      return (control?.networkConnections ?? []) as T
    case 'network_delete_paths':
      return (Array.isArray(args?.paths) ? args.paths.filter(path => typeof path === 'string' && path.includes('/gvfs/')) : []) as T
    case 'can_trash_paths':
      return (control?.trashSupported !== false && (control?.networkTrashSupported !== false ||
        !(Array.isArray(args?.paths) && args.paths.some(path => String(path).includes('/gvfs/'))))) as T
    case 'network_delete_entries':
      if (args?.trash === true && control?.networkTrashSupported === false && args?.confirmed !== true) throw { code: 'network_confirmation_required', message: 'Network trash is unsupported.' }
      if (args?.trash !== true && args?.confirmed !== true) throw { code: 'network_confirmation_required', message: 'Network deletion requires confirmation.' }
      while (control?.networkDeleteHold) await new Promise(resolve => setTimeout(resolve, 20))
      return undefined as T
    case 'forget_network_connection':
      if (control) control.networkConnections = control.networkConnections?.filter(connection => connection.uri !== args?.uri)
      return undefined as T
    case 'list_network_entries':
      return (control?.networkConnections ?? []).map(connection => ({
        name: connection.label, path: connection.uri, kind: 'dir', iconId: 23, network: true,
      })) as T
    case 'resolve_mounted_path_for_uri':
      return (control?.networkMountedPaths?.[String(args?.uri)] ?? null) as T
    case 'classify_network_uri': {
      const address = String(args?.uri ?? '').trim()
      const rawScheme = address.split('://')[0].toLowerCase()
      if (!address.includes('://')) return { kind: 'not_uri', scheme: null, normalizedUri: null } as T
      const scheme = rawScheme === 'ssh' ? 'sftp' : rawScheme
      const kind = ['sftp', 'smb', 'nfs', 'ftp', 'dav', 'davs', 'afp', 'mtp'].includes(scheme) ? 'mountable'
        : ['http', 'https'].includes(scheme) ? 'external' : 'unsupported'
      return { kind, scheme, normalizedUri: address.replace(`${rawScheme}://`, `${scheme}://`) } as T
    }
    case 'compress_entries':
      return `/mock/${args?.name}` as T
    case 'extract_archive':
      if (control?.archivePassword !== undefined && args?.password !== control.archivePassword) {
        throw { code: args?.password === undefined ? 'archive_password_required' : 'archive_invalid_password', message: args?.password === undefined ? 'Password required' : 'Incorrect archive password' }
      }
      return { destination: '/mock/extracted', skipped_symlinks: 0, skipped_entries: 0 } as T
    case 'list_open_with_apps':
      return [
        { id: 'alpha', name: 'Alpha editor', exec: 'alpha', matches: true, terminal: false, defaultContentType: 'text/plain' },
        { id: 'beta', name: 'Beta editor', exec: 'beta', matches: false, terminal: false, defaultContentType: 'text/plain' },
      ] as T
    case 'cloud_writeback_statuses':
      return (control?.cloudStatuses ?? []) as T
    case 'save_cloud_working_copy': {
      if (control?.cloudSaveError) throw control.cloudSaveError
      const copy = control?.cloudCopies?.find(copy => copy.id === args?.id)
      if (copy) copy.dirty = false
      return { ...copy, saveStatus: 'saved' } as T
    }
    case 'set_cloud_working_copy_auto_save':
      return undefined as T
    case 'list_cloud_working_copies':
      return (control?.cloudCopies ?? []) as T
    case 'cloud_working_copy_overview': {
      const copies = (control?.cloudCopies ?? []).map(copy => ({ ...copy,
        storageBytes: copy.storageBytes ?? copy.originalSize ?? 0,
        cleanupBlockedReason: copy.cleanupBlockedReason ?? (copy.dirty ? 'New local changes were found; save them before removing this copy' : null),
      }))
      return { copies, storageBytes: copies.reduce((sum, copy) => sum + copy.storageBytes, 0), incomplete: false, retainedEntries: 0 } as T
    }
    case 'remove_cloud_working_copies': {
      if (args?.editorsClosed !== true) throw { code: 'conflict', message: 'Close the editor first' }
      const removedIds: string[] = []
      const skipped: Array<{ id: string; reason: string }> = []
      for (const id of new Set(args?.ids as string[])) {
        const copy = control?.cloudCopies?.find(copy => copy.id === id)
        if (!copy || copy.dirty || copy.cleanupBlockedReason || ['pending', 'uploading', 'error', 'conflict'].includes(copy.saveStatus ?? 'manual')) {
          skipped.push({ id, reason: 'New changes or unresolved saves were found; this copy was kept' })
        } else {
          removedIds.push(id)
          if (control) {
            control.cloudCopies = control.cloudCopies?.filter(copy => copy.id !== id)
            control.cloudStatuses = control.cloudStatuses?.filter(row => (row as { id: string }).id !== id)
          }
          emitMockEvent('cloud-working-copy-removed', id)
        }
      }
      return { removedIds, skipped } as T
    }
    case 'cloud_working_copy_storage_path':
      return '/mock/browsey/cloud-workspaces' as T
    case 'prepare_cloud_working_copy':
      return { id: '1-2-3', sourcePath: args?.path, localPath: '/mock/browsey/cloud-workspaces/1-2-3/files/report.txt', dirty: false } as T
    case 'upload_cloud_working_copy':
      return { path: 'rclone://test/report-edited-1-2-3.txt', sourceChanged: control?.cloudUploadChanged ?? false } as T
    case 'prepare_cloud_external_copy':
      return ['/mock/browsey/cloud-workspaces/export/inputs/report.txt'] as T
    case 'open_with':
    case 'set_default_app':
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
        filesystem: args?.filesystem === 'ntfs' ? 'NTFS' : args?.filesystem ?? 'exFAT',
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

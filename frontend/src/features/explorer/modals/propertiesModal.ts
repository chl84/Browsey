import { writable, get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'

type AccessBit = boolean | 'mixed'
type Access = { read: AccessBit; write: AccessBit; exec: AccessBit }
type OwnershipPrincipalKind = 'user' | 'group'
type PermissionPayload = {
  access_supported: boolean
  executable_supported: boolean
  ownership_supported?: boolean
  read_only: boolean
  executable: boolean | null
  owner_name?: string | null
  group_name?: string | null
  owner?: Access
  group?: Access
  other?: Access
}

export type ExtraMetadataField = {
  key: string
  label: string
  value: string
}

export type ExtraMetadataSection = {
  id: string
  title: string
  fields: ExtraMetadataField[]
}

export type ExtraMetadataPayload = {
  kind: string
  sections: ExtraMetadataSection[]
}

const withTrashOriginalPath = (
  metadata: ExtraMetadataPayload,
  entry: Entry,
): ExtraMetadataPayload => {
  const originalPath = typeof entry.original_path === 'string' ? entry.original_path.trim() : ''
  if (!entry.trash_id || !originalPath) return metadata
  const originalPathSection: ExtraMetadataSection = {
    id: 'trash',
    title: 'Trash',
    fields: [{ key: 'original_path', label: 'Original path', value: originalPath }],
  }
  return {
    ...metadata,
    sections: [originalPathSection, ...metadata.sections],
  }
}

const isTrashEntry = (entry: Entry): boolean =>
  typeof entry.trash_id === 'string' && entry.trash_id.trim().length > 0

const isUriPath = (path: string): boolean => {
  const trimmed = path.trim()
  const idx = trimmed.indexOf('://')
  if (idx <= 0) return false
  const scheme = trimmed.slice(0, idx)
  return /^[a-zA-Z][a-zA-Z0-9+.-]*$/.test(scheme)
}

const isVirtualUriEntry = (entry: Entry): boolean => isUriPath(entry.path)

const shouldLockMutations = (entries: Entry[]): boolean =>
  entries.length > 0 && entries.every(isTrashEntry)

export type PermissionsState = {
  accessSupported: boolean
  executableSupported: boolean
  ownershipSupported: boolean
  readOnly: AccessBit | null
  executable: AccessBit | null
  ownerName: string | null
  groupName: string | null
  owner: Access | null
  group: Access | null
  other: Access | null
}

export type PropertiesState = {
  open: boolean
  entry: Entry | null
  targets: Entry[]
  mutationsLocked: boolean
  count: number
  size: number | null
  itemCount: number | null
  hidden: AccessBit | null
  extraMetadataLoading: boolean
  extraMetadataError: string | null
  extraMetadata: ExtraMetadataPayload | null
  extraMetadataPath: string | null
  permissionsLoading: boolean
  permissions: PermissionsState | null
  ownershipUsers: string[]
  ownershipGroups: string[]
  ownershipOptionsLoading: boolean
  ownershipOptionsError: string | null
  ownershipApplying: boolean
  ownershipError: string | null
}

const PERMISSIONS_MULTI_CONCURRENCY = 8
const OWNERSHIP_PRINCIPAL_LIMIT = 2048
const SYMLINK_PERMISSIONS_MSG = 'Permissions are not supported on symlinks'

const unsupportedPermissionsPayload = (): PermissionPayload => ({
  access_supported: false,
  executable_supported: false,
  ownership_supported: false,
  read_only: false,
  executable: null,
  owner_name: null,
  group_name: null,
})

const unsupportedPermissionsState = (): PermissionsState => ({
  accessSupported: false,
  executableSupported: false,
  ownershipSupported: false,
  readOnly: null,
  executable: null,
  ownerName: null,
  groupName: null,
  owner: null,
  group: null,
  other: null,
})

const normalizePrincipalList = (values: unknown): string[] => {
  if (!Array.isArray(values)) return []
  const cleaned = values
    .map((value) => (typeof value === 'string' ? value.trim() : ''))
    .filter((value) => value.length > 0)
  cleaned.sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }))
  return Array.from(new Set(cleaned))
}

const fetchOwnershipPrincipalList = async (
  kind: OwnershipPrincipalKind,
  limit = OWNERSHIP_PRINCIPAL_LIMIT,
): Promise<string[]> => {
  const raw = await invoke<unknown>('list_ownership_principals', { kind, limit })
  return normalizePrincipalList(raw)
}

const invokeErrorMessage = (err: unknown): string => {
  if (err instanceof Error && err.message.trim().length > 0) return err.message
  if (typeof err === 'string' && err.trim().length > 0) return err
  if (err && typeof err === 'object') {
    const record = err as Record<string, unknown>
    const candidates = [record.message, record.error, record.cause]
    for (const candidate of candidates) {
      if (typeof candidate === 'string' && candidate.trim().length > 0) return candidate
    }
    try {
      const serialized = JSON.stringify(err)
      if (serialized && serialized !== '{}') return serialized
    } catch {
      // Ignore serialization issues and use fallback.
    }
  }
  return 'Unknown error'
}

const isExpectedOwnershipError = (message: string): boolean => {
  const normalized = message.toLowerCase()
  return (
    normalized.includes('request dismissed') ||
    normalized.includes('authentication was cancelled') ||
    normalized.includes('cancelled or denied') ||
    normalized.includes('user not found') ||
    normalized.includes('group not found') ||
    normalized.includes('operation not permitted') ||
    normalized.includes('requires elevated privileges') ||
    normalized.includes('permission denied')
  )
}

const isExpectedPermissionUpdateError = (message: string): boolean => {
  const normalized = message.toLowerCase()
  return (
    normalized.includes('operation not permitted') ||
    normalized.includes('permission denied') ||
    normalized.includes('read-only file system') ||
    normalized.includes('access is denied') ||
    normalized.includes('failed to update permissions')
  )
}

type Deps = {
  computeDirStats: (
    paths: string[],
    onProgress?: (bytes: number, items: number) => void,
  ) => Promise<{ total: number; items: number }>
  showToast: (msg: string, timeout?: number) => void
}

export const createPropertiesModal = (deps: Deps) => {
  const { computeDirStats, showToast } = deps
  const state = writable<PropertiesState>({
    open: false,
    entry: null,
    targets: [],
    mutationsLocked: false,
    count: 0,
    size: null,
    itemCount: null,
    hidden: null,
    extraMetadataLoading: false,
    extraMetadataError: null,
    extraMetadata: null,
    extraMetadataPath: null,
    permissionsLoading: false,
    permissions: null,
    ownershipUsers: [],
    ownershipGroups: [],
    ownershipOptionsLoading: false,
    ownershipOptionsError: null,
    ownershipApplying: false,
    ownershipError: null,
  })
  let token = 0
  let ownershipPrincipalsLoadedToken = -1
  let lastPermissionsErrorSignature = ''
  let lastPermissionsErrorAt = 0

  const close = () => {
    state.set({
      open: false,
      entry: null,
      targets: [],
      mutationsLocked: false,
      count: 0,
      size: null,
      itemCount: null,
      hidden: null,
      extraMetadataLoading: false,
      extraMetadataError: null,
      extraMetadata: null,
      extraMetadataPath: null,
      permissionsLoading: false,
      permissions: null,
      ownershipUsers: [],
      ownershipGroups: [],
      ownershipOptionsLoading: false,
      ownershipOptionsError: null,
      ownershipApplying: false,
      ownershipError: null,
    })
    ownershipPrincipalsLoadedToken = -1
  }

  const openModal = async (entries: Entry[]) => {
    const nextToken = ++token
    ownershipPrincipalsLoadedToken = -1
    const files = entries.filter((e) => e.kind === 'file')
    const dirs = entries.filter((e) => e.kind === 'dir')
    const localDirs = dirs.filter((e) => !isVirtualUriEntry(e))
    const singleVirtualUri = entries.length === 1 && isVirtualUriEntry(entries[0])
    const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)
    const fileCount = files.length

    state.set({
      open: true,
      entry: entries.length === 1 ? entries[0] : null,
      targets: entries,
      mutationsLocked: shouldLockMutations(entries),
      count: entries.length,
      size: fileBytes,
      itemCount: dirs.length === 0 ? fileCount : null,
      hidden: combine(entries.map((e) => e.hidden == true)),
      extraMetadataLoading: false,
      extraMetadataError: null,
      extraMetadata: null,
      extraMetadataPath: null,
      permissionsLoading: !singleVirtualUri,
      permissions: singleVirtualUri ? unsupportedPermissionsState() : null,
      ownershipUsers: [],
      ownershipGroups: [],
      ownershipOptionsLoading: false,
      ownershipOptionsError: null,
      ownershipApplying: false,
      ownershipError: null,
    })

    if (entries.length === 1) {
      const entry = entries[0]
      if (!singleVirtualUri) {
        void loadPermissions(entry, nextToken)
        void loadEntryTimes(entry, nextToken)
      }
    } else {
      void loadPermissionsMulti(entries, nextToken)
    }

    if (localDirs.length > 0) {
      const { total, items } = await computeDirStats(
        localDirs.map((d) => d.path),
        (partialBytes) => {
          if (nextToken === token) {
            state.update((s) => ({ ...s, size: fileBytes + partialBytes }))
          }
        },
      )
      if (nextToken === token) {
        state.update((s) => ({ ...s, size: fileBytes + total, itemCount: fileCount + items }))
      }
    } else if (dirs.length === 0) {
      state.update((s) => ({ ...s, itemCount: fileCount }))
    }
  }

  const combine = (values: boolean[]): AccessBit => {
    if (values.length === 0) return false
    const allTrue = values.every((v) => v === true)
    const allFalse = values.every((v) => v === false)
    if (allTrue) return true
    if (allFalse) return false
    return 'mixed'
  }

  const combinePrincipal = (values: Array<string | null | undefined>): string | null => {
    if (values.length === 0) return null
    const normalized = values.map((v) => (typeof v === 'string' ? v.trim() : ''))
    const unique = Array.from(new Set(normalized.filter((v) => v.length > 0)))
    if (unique.length === 0) return null
    if (unique.length === 1 && normalized.every((v) => v === unique[0])) return unique[0]
    return 'mixed'
  }

  const ensureOwnershipPrincipalsLoaded = async (currToken: number) => {
    if (currToken !== token) return
    if (ownershipPrincipalsLoadedToken === currToken) return
    const current = get(state)
    if (!current.open || current.ownershipOptionsLoading) return
    state.update((s) => ({ ...s, ownershipOptionsLoading: true, ownershipOptionsError: null }))
    try {
      const [users, groups] = await Promise.all([
        fetchOwnershipPrincipalList('user'),
        fetchOwnershipPrincipalList('group'),
      ])
      if (currToken !== token) return
      state.update((s) => ({
        ...s,
        ownershipUsers: users,
        ownershipGroups: groups,
        ownershipOptionsLoading: false,
        ownershipOptionsError: null,
      }))
    } catch (err) {
      if (currToken !== token) return
      const message = invokeErrorMessage(err)
      console.warn('Failed to load ownership principals', message)
      state.update((s) => ({
        ...s,
        ownershipUsers: [],
        ownershipGroups: [],
        ownershipOptionsLoading: false,
        ownershipOptionsError: message,
      }))
    } finally {
      if (currToken === token) {
        ownershipPrincipalsLoadedToken = currToken
      }
    }
  }

  const fetchPermissionsAll = async (entries: Entry[]): Promise<PermissionPayload[]> => {
    if (entries.length === 0) return []

    const results: PermissionPayload[] = new Array(entries.length)
    const workerCount = Math.min(PERMISSIONS_MULTI_CONCURRENCY, entries.length)
    let nextIdx = 0
    let failures = 0
    let unexpectedFailures = 0

    const worker = async () => {
      while (true) {
        const idx = nextIdx
        nextIdx += 1
        if (idx >= entries.length) {
          return
        }
        const e = entries[idx]
        if (isVirtualUriEntry(e)) {
          results[idx] = unsupportedPermissionsPayload()
          continue
        }
        try {
          results[idx] = await invoke<PermissionPayload>('get_permissions', { path: e.path })
        } catch (err) {
          failures += 1
          const msg = err instanceof Error ? err.message : String(err)
          if (!msg.includes(SYMLINK_PERMISSIONS_MSG)) {
            unexpectedFailures += 1
          }
          results[idx] = unsupportedPermissionsPayload()
        }
      }
    }

    await Promise.all(Array.from({ length: workerCount }, () => worker()))
    if (unexpectedFailures > 0) {
      console.warn(
        `Permissions unavailable for ${failures} selected item(s), ${unexpectedFailures} unexpected`,
      )
    }
    return results
  }

  const loadPermissionsMulti = async (entries: Entry[], currToken: number) => {
    try {
      const permsList = await fetchPermissionsAll(entries)
      if (currToken !== token) return
      const accessSupported = permsList.every((p) => p.access_supported)
      const executableSupported = permsList.every((p) => p.executable_supported)
      const ownershipSupported = permsList.every((p) => p.ownership_supported === true)

      const ownerReads = permsList
        .map((p) => p.owner?.read)
        .filter((v): v is boolean => v !== undefined)
      const ownerWrites = permsList
        .map((p) => p.owner?.write)
        .filter((v): v is boolean => v !== undefined)
      const ownerExecs = permsList
        .map((p) => p.owner?.exec)
        .filter((v): v is boolean => v !== undefined)

      const groupReads = permsList
        .map((p) => p.group?.read)
        .filter((v): v is boolean => v !== undefined)
      const groupWrites = permsList
        .map((p) => p.group?.write)
        .filter((v): v is boolean => v !== undefined)
      const groupExecs = permsList
        .map((p) => p.group?.exec)
        .filter((v): v is boolean => v !== undefined)

      const otherReads = permsList
        .map((p) => p.other?.read)
        .filter((v): v is boolean => v !== undefined)
      const otherWrites = permsList
        .map((p) => p.other?.write)
        .filter((v): v is boolean => v !== undefined)
      const otherExecs = permsList
        .map((p) => p.other?.exec)
        .filter((v): v is boolean => v !== undefined)

      const readOnlyVals = permsList.map((p) => p.read_only)
      const execVals = permsList
        .map((p) => p.executable)
        .filter((v): v is boolean => v !== null && v !== undefined)
      const ownerNameVals = permsList.map((p) => p.owner_name ?? null)
      const groupNameVals = permsList.map((p) => p.group_name ?? null)

      state.update((s) => ({
        ...s,
        permissionsLoading: false,
        permissions: accessSupported
          ? {
              accessSupported,
              executableSupported,
              ownershipSupported,
              readOnly: combine(readOnlyVals),
              executable: executableSupported ? combine(execVals) : null,
              ownerName: combinePrincipal(ownerNameVals),
              groupName: combinePrincipal(groupNameVals),
              owner: {
                read: combine(ownerReads),
                write: combine(ownerWrites),
                exec: combine(ownerExecs),
              },
              group: {
                read: combine(groupReads),
                write: combine(groupWrites),
                exec: combine(groupExecs),
              },
              other: {
                read: combine(otherReads),
                write: combine(otherWrites),
                exec: combine(otherExecs),
              },
            }
          : {
              accessSupported: false,
              executableSupported,
              ownershipSupported,
              readOnly: null,
              executable: executableSupported ? combine(execVals) : null,
              ownerName: combinePrincipal(ownerNameVals),
              groupName: combinePrincipal(groupNameVals),
              owner: null,
              group: null,
              other: null,
            },
      }))
      if (ownershipSupported) {
        void ensureOwnershipPrincipalsLoaded(currToken)
      }
    } catch (err) {
      console.error('Failed to load multi permissions', err)
      if (currToken !== token) return
      state.update((s) => ({ ...s, permissionsLoading: false }))
    }
  }

  const loadPermissions = async (entry: Entry, currToken: number) => {
    try {
      const perms = await invoke<PermissionPayload>('get_permissions', { path: entry.path })
      if (currToken !== token) return
      state.update((s) => ({
        ...s,
        permissionsLoading: false,
        permissions: {
          accessSupported: perms.access_supported,
          executableSupported: perms.executable_supported,
          ownershipSupported: perms.ownership_supported === true,
          readOnly: perms.read_only,
          executable: perms.executable,
          ownerName: perms.owner_name ?? null,
          groupName: perms.group_name ?? null,
          owner: perms.owner ?? null,
          group: perms.group ?? null,
          other: perms.other ?? null,
        },
      }))
      if (perms.ownership_supported === true) {
        void ensureOwnershipPrincipalsLoaded(currToken)
      }
    } catch (err) {
      console.error('Failed to load permissions', err)
      if (currToken !== token) return
      state.update((s) => ({ ...s, permissionsLoading: false }))
    }
  }

  const loadEntryTimes = async (entry: Entry, currToken: number) => {
    try {
      const times = await invoke<{
        accessed?: string | null
        created?: string | null
        modified?: string | null
      }>('entry_times_cmd', { path: entry.path })
      if (currToken !== token) return
      state.update((s) => ({
        ...s,
        entry: { ...entry, ...times },
      }))
    } catch (err) {
      console.error('Failed to load entry times', err)
    }
  }

  const loadExtraMetadata = async (entry: Entry, currToken: number) => {
    try {
      const metadata = await invoke<ExtraMetadataPayload>('entry_extra_metadata_cmd', { path: entry.path })
      if (currToken !== token) return
      const metadataWithTrashPath = withTrashOriginalPath(metadata, entry)
      state.update((s) => ({
        ...s,
        extraMetadataLoading: false,
        extraMetadataError: null,
        extraMetadata: metadataWithTrashPath,
        extraMetadataPath: entry.path,
      }))
    } catch (err) {
      if (currToken !== token) return
      state.update((s) => ({
        ...s,
        extraMetadataLoading: false,
        extraMetadataError: err instanceof Error ? err.message : String(err),
        extraMetadata: null,
        extraMetadataPath: null,
      }))
    }
  }

  const loadExtraIfNeeded = () => {
    const current = get(state)
    if (!current.open || current.count !== 1 || !current.entry) return
    if (current.extraMetadataLoading) return
    if (current.extraMetadata && current.extraMetadataPath === current.entry.path) return
    const activeToken = token
    const entry = current.entry
    state.update((s) => ({
      ...s,
      extraMetadataLoading: true,
      extraMetadataError: null,
    }))
    void loadExtraMetadata(entry, activeToken)
  }

  const updatePermissions = async (
    opts: {
      readOnly?: boolean
      executable?: boolean
      owner?: Partial<Access>
      group?: Partial<Access>
      other?: Partial<Access>
    },
    prevState?: PermissionsState | null,
  ) => {
    const current = get(state)
    if (current.mutationsLocked) return
    const targets =
      current.targets.length > 0
        ? current.targets.map((p) => p.path)
        : current.entry
          ? [current.entry.path]
          : []
    if (targets.length === 0) {
      console.warn('updatePermissions called with no targets', opts)
      return
    }
    const payload: {
      paths: string[]
      readOnly?: boolean
      executable?: boolean
      owner?: Partial<Access>
      group?: Partial<Access>
      other?: Partial<Access>
    } = { paths: targets }
    if (opts.readOnly !== undefined) payload.readOnly = opts.readOnly
    if (opts.executable !== undefined) payload.executable = opts.executable
    if (opts.owner && Object.keys(opts.owner).length > 0) payload.owner = { ...opts.owner }
    if (opts.group && Object.keys(opts.group).length > 0) payload.group = { ...opts.group }
    if (opts.other && Object.keys(opts.other).length > 0) payload.other = { ...opts.other }
    const activeToken = token

    try {
      const perms = await invoke<{
        access_supported?: boolean
        executable_supported?: boolean
        ownership_supported?: boolean
        read_only?: boolean
        executable?: boolean | null
        owner_name?: string | null
        group_name?: string | null
        owner?: Access
        group?: Access
        other?: Access
      }>('set_permissions', payload)
      if (targets.length > 1) {
        state.update((s) => ({ ...s, permissionsLoading: true }))
        await loadPermissionsMulti(current.targets, activeToken)
        return
      }
      const currentPerms = get(state).permissions
      state.update((s) => ({
        ...s,
        permissions: {
          accessSupported: perms.access_supported ?? currentPerms?.accessSupported ?? false,
          executableSupported: perms.executable_supported ?? currentPerms?.executableSupported ?? false,
          ownershipSupported: perms.ownership_supported ?? currentPerms?.ownershipSupported ?? false,
          readOnly: perms.read_only ?? currentPerms?.readOnly ?? null,
          executable: perms.executable ?? currentPerms?.executable ?? null,
          ownerName: perms.owner_name ?? currentPerms?.ownerName ?? null,
          groupName: perms.group_name ?? currentPerms?.groupName ?? null,
          owner: perms.owner ?? currentPerms?.owner ?? null,
          group: perms.group ?? currentPerms?.group ?? null,
          other: perms.other ?? currentPerms?.other ?? null,
        },
      }))
    } catch (err) {
      if (activeToken !== token) return
      const message = invokeErrorMessage(err)
      const signature = `${targets.join('\n')}|${JSON.stringify(opts)}|${message}`
      const now = Date.now()
      const duplicate =
        signature === lastPermissionsErrorSignature && now - lastPermissionsErrorAt < 2000

      if (!duplicate) {
        lastPermissionsErrorSignature = signature
        lastPermissionsErrorAt = now

        if (!isExpectedPermissionUpdateError(message)) {
          console.error('Failed to update permissions', { targets, opts, message, err })
        }
      }

      if (prevState) {
        state.update((s) => ({ ...s, permissions: prevState }))
      }
    }
  }

  const toggleAccess = (scope: 'owner' | 'group' | 'other', key: 'read' | 'write' | 'exec', next: boolean) => {
    const current = get(state)
    if (current.mutationsLocked) return
    const perms = current.permissions
    if (!perms || !perms.accessSupported) return
    const prev = JSON.parse(JSON.stringify(perms)) as PermissionsState
    const updatedScope = perms[scope] ? { ...perms[scope], [key]: next } : perms[scope]
    const updated = { ...perms, [scope]: updatedScope }
    state.update((s) => ({ ...s, permissions: updated }))
    void updatePermissions({ [scope]: { [key]: next } }, prev)
  }

  const setOwnership = async (ownerRaw: string, groupRaw: string) => {
    const current = get(state)
    if (current.mutationsLocked) return
    const targets =
      current.targets.length > 0
        ? current.targets.map((p) => p.path)
        : current.entry
          ? [current.entry.path]
          : []
    if (targets.length === 0) return

    const owner = ownerRaw.trim()
    const group = groupRaw.trim()
    const payload: {
      paths: string[]
      owner?: string
      group?: string
    } = { paths: targets }
    if (owner.length > 0) payload.owner = owner
    if (group.length > 0) payload.group = group
    if (!payload.owner && !payload.group) {
      state.update((s) => ({ ...s, ownershipError: 'Specify user and/or group before applying.' }))
      return
    }

    const activeToken = token
    state.update((s) => ({ ...s, ownershipApplying: true, ownershipError: null }))
    try {
      const perms = await invoke<PermissionPayload>('set_ownership', payload)
      if (activeToken !== token) return
      if (targets.length > 1) {
        state.update((s) => ({ ...s, permissionsLoading: true }))
        await loadPermissionsMulti(current.targets, activeToken)
        return
      }
      const currentPerms = get(state).permissions
      state.update((s) => ({
        ...s,
        permissions: {
          accessSupported: perms.access_supported ?? currentPerms?.accessSupported ?? false,
          executableSupported: perms.executable_supported ?? currentPerms?.executableSupported ?? false,
          ownershipSupported: perms.ownership_supported ?? currentPerms?.ownershipSupported ?? false,
          readOnly: perms.read_only ?? currentPerms?.readOnly ?? null,
          executable: perms.executable ?? currentPerms?.executable ?? null,
          ownerName: perms.owner_name ?? currentPerms?.ownerName ?? null,
          groupName: perms.group_name ?? currentPerms?.groupName ?? null,
          owner: perms.owner ?? currentPerms?.owner ?? null,
          group: perms.group ?? currentPerms?.group ?? null,
          other: perms.other ?? currentPerms?.other ?? null,
        },
      }))
    } catch (err) {
      if (activeToken !== token) return
      const rawMessage = invokeErrorMessage(err)
      const message = rawMessage.toLowerCase().includes('operation not permitted')
        ? 'Permission denied. Changing owner/group requires elevated privileges.'
        : rawMessage
      if (!isExpectedOwnershipError(rawMessage) && !isExpectedOwnershipError(message)) {
        console.warn('Ownership update failed:', message)
      }
      state.update((s) => ({ ...s, ownershipError: message }))
    } finally {
      if (activeToken !== token) return
      state.update((s) => ({ ...s, ownershipApplying: false }))
    }
  }

  return {
    state,
    open: openModal,
    close,
    loadExtraIfNeeded,
    toggleAccess,
    setOwnership,
    async toggleHidden(next: boolean) {
      const current = get(state)
      if (current.mutationsLocked) return
      const targets = current.targets.length > 0 ? current.targets : current.entry ? [current.entry] : []
      if (targets.length === 0) return
      const failures: string[] = []
      const newPaths: string[] = []
      const successFlags: boolean[] = []
      for (const t of targets) {
        try {
          const res = await invoke<string[]>('set_hidden', { paths: [t.path], hidden: next })
          const np = res[0] ?? t.path
          newPaths.push(np)
          successFlags.push(true)
        } catch (err) {
          console.error('Failed to toggle hidden', err)
          failures.push(t.name)
          newPaths.push(t.path)
          successFlags.push(false)
        }
      }
      state.update((s) => {
        const updatedTargets = s.targets.map((t, idx) => {
          const np = newPaths[idx] ?? t.path
          return successFlags[idx] ? { ...t, path: np, name: stdPathName(np), hidden: next } : t
        })
        const updatedEntry =
          s.entry && newPaths.length > 0
            ? successFlags[0]
              ? { ...s.entry, path: newPaths[0], name: stdPathName(newPaths[0]), hidden: next }
              : s.entry
            : s.entry
        const hiddenBits = updatedTargets.map((t) => t.hidden === true)
        return { ...s, targets: updatedTargets, entry: updatedEntry, hidden: combine(hiddenBits) }
      })
      if (failures.length > 0) {
        showToast(`Hidden toggle skipped for: ${failures.join(', ')}`)
      }
    },
  }
}

const stdPathName = (p: string): string => {
  const parts = p.split(/[\\/]/)
  return parts[parts.length - 1] ?? p
}

import { writable, get } from 'svelte/store'
import { invoke } from '@/shared/lib/tauri'
import type { Entry } from '../model/types'

type AccessBit = boolean | 'mixed'
type Access = { read: AccessBit; write: AccessBit; exec: AccessBit }
type OwnershipPrincipalKind = 'user' | 'group'
type InvokeApiError = { code: string; message: string }
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

type PermissionBatchItemPayload = {
  path: string
  ok: boolean
  permissions: PermissionPayload
  error?: InvokeApiError | string | null
}

type PermissionBatchAggregatePayload = {
  access_supported: boolean
  executable_supported: boolean
  ownership_supported: boolean
  read_only: AccessBit | null
  executable: AccessBit | null
  owner_name?: string | null
  group_name?: string | null
  owner?: Access | null
  group?: Access | null
  other?: Access | null
}

type PermissionBatchPayload = {
  per_item: PermissionBatchItemPayload[]
  aggregate: PermissionBatchAggregatePayload
  failures: number
  unexpected_failures: number
}

type HiddenBatchItemPayload = {
  path: string
  ok: boolean
  new_path: string
  error?: InvokeApiError | string | null
}

type HiddenBatchPayload = {
  per_item: HiddenBatchItemPayload[]
  failures: number
  unexpected_failures: number
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
  ownershipSupported: boolean
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
  permissionsApplying: boolean
  permissions: PermissionsState | null
  ownershipUsers: string[]
  ownershipGroups: string[]
  ownershipOptionsLoading: boolean
  ownershipOptionsError: string | null
  ownershipApplying: boolean
  ownershipError: string | null
}

const OWNERSHIP_PRINCIPAL_LIMIT = 2048

const unsupportedPermissionsState = (): PermissionsState => ({
  accessSupported: false,
  ownershipSupported: false,
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
    const nestedError =
      record.error && typeof record.error === 'object'
        ? (record.error as Record<string, unknown>)
        : null
    const candidates = [
      record.message,
      record.error,
      record.cause,
      nestedError?.message,
      nestedError?.error,
      nestedError?.cause,
    ]
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

const invokeErrorCode = (err: unknown): string | null => {
  if (typeof err === 'string' && err.trim().length > 0) {
    try {
      const parsed = JSON.parse(err) as Record<string, unknown>
      if (typeof parsed.code === 'string' && parsed.code.trim().length > 0) {
        return parsed.code.trim()
      }
    } catch {
      // Ignore parse failures for plain strings.
    }
  }
  if (!err || typeof err !== 'object') return null
  const record = err as Record<string, unknown>
  if (typeof record.code === 'string' && record.code.trim().length > 0) {
    return record.code.trim()
  }
  if (record.error && typeof record.error === 'object') {
    const nested = record.error as Record<string, unknown>
    if (typeof nested.code === 'string' && nested.code.trim().length > 0) {
      return nested.code.trim()
    }
  }
  return null
}

const isExpectedOwnershipError = (code: string | null): boolean => {
  return (
    code === 'authentication_cancelled' ||
    code === 'principal_not_found' ||
    code === 'permission_denied' ||
    code === 'elevated_required' ||
    code === 'unsupported_platform'
  )
}

const isExpectedPermissionUpdateError = (code: string | null): boolean => {
  return (
    code === 'permission_denied' ||
    code === 'elevated_required' ||
    code === 'read_only_filesystem' ||
    code === 'symlink_unsupported'
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
    permissionsApplying: false,
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
      permissionsApplying: false,
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
      permissionsApplying: false,
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

  const loadPermissionsMulti = async (entries: Entry[], currToken: number) => {
    try {
      const paths = entries.map((entry) => entry.path)
      const batch = await invoke<PermissionBatchPayload>('get_permissions_batch', { paths })
      if (currToken !== token) return
      if (batch.unexpected_failures > 0) {
        console.warn(
          `Permissions unavailable for ${batch.failures} selected item(s), ${batch.unexpected_failures} unexpected`,
        )
      }

      const aggregate = batch.aggregate

      state.update((s) => ({
        ...s,
        permissionsLoading: false,
        permissions: {
          accessSupported: aggregate.access_supported,
          ownershipSupported: aggregate.ownership_supported,
          ownerName: aggregate.owner_name ?? null,
          groupName: aggregate.group_name ?? null,
          owner: aggregate.owner ?? null,
          group: aggregate.group ?? null,
          other: aggregate.other ?? null,
        },
      }))
      if (aggregate.ownership_supported) {
        void ensureOwnershipPrincipalsLoaded(currToken)
      }
    } catch (err) {
      const code = invokeErrorCode(err)
      const message = invokeErrorMessage(err)
      console.error(
        `Failed to load multi permissions${code ? ` [${code}]` : ''}: ${message}`,
      )
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
          ownershipSupported: perms.ownership_supported === true,
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
      const code = invokeErrorCode(err)
      const message = invokeErrorMessage(err)
      console.error(`Failed to load permissions${code ? ` [${code}]` : ''}: ${message}`)
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
      const code = invokeErrorCode(err)
      const message = invokeErrorMessage(err)
      console.error(`Failed to load entry times${code ? ` [${code}]` : ''}: ${message}`)
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
      const message = invokeErrorMessage(err)
      state.update((s) => ({
        ...s,
        extraMetadataLoading: false,
        extraMetadataError: message,
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
    state.update((s) => ({ ...s, permissionsApplying: true }))

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
        await loadPermissionsMulti(current.targets, activeToken)
        return
      }
      const currentPerms = get(state).permissions
      state.update((s) => ({
        ...s,
        permissions: {
          accessSupported: perms.access_supported ?? currentPerms?.accessSupported ?? false,
          ownershipSupported: perms.ownership_supported ?? currentPerms?.ownershipSupported ?? false,
          ownerName: perms.owner_name ?? currentPerms?.ownerName ?? null,
          groupName: perms.group_name ?? currentPerms?.groupName ?? null,
          owner: perms.owner ?? currentPerms?.owner ?? null,
          group: perms.group ?? currentPerms?.group ?? null,
          other: perms.other ?? currentPerms?.other ?? null,
        },
      }))
    } catch (err) {
      if (activeToken !== token) return
      const code = invokeErrorCode(err)
      const message = invokeErrorMessage(err)
      const signature = `${targets.join('\n')}|${JSON.stringify(opts)}|${code ?? ''}|${message}`
      const now = Date.now()
      const duplicate =
        signature === lastPermissionsErrorSignature && now - lastPermissionsErrorAt < 2000

      if (!duplicate) {
        lastPermissionsErrorSignature = signature
        lastPermissionsErrorAt = now

        if (!isExpectedPermissionUpdateError(code)) {
          console.error('Failed to update permissions', { targets, opts, message, err })
        }
      }

      if (prevState) {
        state.update((s) => ({ ...s, permissions: prevState }))
      }
    } finally {
      if (activeToken === token) {
        state.update((s) => ({ ...s, permissionsApplying: false }))
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
          ownershipSupported: perms.ownership_supported ?? currentPerms?.ownershipSupported ?? false,
          ownerName: perms.owner_name ?? currentPerms?.ownerName ?? null,
          groupName: perms.group_name ?? currentPerms?.groupName ?? null,
          owner: perms.owner ?? currentPerms?.owner ?? null,
          group: perms.group ?? currentPerms?.group ?? null,
          other: perms.other ?? currentPerms?.other ?? null,
        },
      }))
    } catch (err) {
      if (activeToken !== token) return
      const code = invokeErrorCode(err)
      const rawMessage = invokeErrorMessage(err)
      const message =
        code === 'permission_denied' ||
        code === 'elevated_required'
        ? 'Permission denied. Changing owner/group requires elevated privileges.'
        : rawMessage
      if (!isExpectedOwnershipError(code)) {
        console.warn('Ownership update failed:', message)
      }
      state.update((s) => ({ ...s, ownershipError: message }))
    } finally {
      if (activeToken === token) {
        state.update((s) => ({ ...s, ownershipApplying: false }))
      }
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
      const activeToken = token

      let batch: HiddenBatchPayload
      try {
        batch = await invoke<HiddenBatchPayload>('set_hidden', {
          paths: targets.map((t) => t.path),
          hidden: next,
        })
      } catch (err) {
        if (activeToken !== token) return
        console.error('Failed to toggle hidden', err)
        showToast('Failed to toggle hidden')
        return
      }

      if (activeToken !== token) return
      if (batch.unexpected_failures > 0) {
        console.warn(
          `Hidden toggle failures: ${batch.failures} selected item(s), ${batch.unexpected_failures} unexpected`,
        )
      }

      const results = Array.isArray(batch.per_item) ? batch.per_item : []
      const failures = targets
        .map((target, idx) => ({ target, result: results[idx] }))
        .filter(({ result }) => !result || !result.ok)
        .map(({ target }) => target.name)

      state.update((s) => {
        const updatedTargets = s.targets.map((t, idx) => {
          const result = results[idx]
          if (!result || !result.ok) return t
          const np = result.new_path || t.path
          return { ...t, path: np, name: stdPathName(np), hidden: next }
        })
        const firstResult = results[0]
        const updatedEntry =
          s.entry && firstResult
            ? firstResult.ok
              ? {
                  ...s.entry,
                  path: firstResult.new_path || s.entry.path,
                  name: stdPathName(firstResult.new_path || s.entry.path),
                  hidden: next,
                }
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

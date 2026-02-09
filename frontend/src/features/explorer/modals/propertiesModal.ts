import { writable, get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'

type AccessBit = boolean | 'mixed'
type Access = { read: AccessBit; write: AccessBit; exec: AccessBit }
type PermissionPayload = {
  access_supported: boolean
  executable_supported: boolean
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

export type PermissionsState = {
  accessSupported: boolean
  executableSupported: boolean
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
  count: number
  size: number | null
  itemCount: number | null
  hidden: AccessBit | null
  extraMetadataLoading: boolean
  extraMetadataError: string | null
  extraMetadata: ExtraMetadataPayload | null
  permissionsLoading: boolean
  permissions: PermissionsState | null
}

const PERMISSIONS_MULTI_CONCURRENCY = 8
const SYMLINK_PERMISSIONS_MSG = 'Permissions are not supported on symlinks'

const unsupportedPermissionsPayload = (): PermissionPayload => ({
  access_supported: false,
  executable_supported: false,
  read_only: false,
  executable: null,
  owner_name: null,
  group_name: null,
})

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
    count: 0,
    size: null,
    itemCount: null,
    hidden: null,
    extraMetadataLoading: false,
    extraMetadataError: null,
    extraMetadata: null,
    permissionsLoading: false,
    permissions: null,
  })
  let token = 0

  const close = () => {
    state.set({
      open: false,
      entry: null,
      targets: [],
      count: 0,
      size: null,
      itemCount: null,
      hidden: null,
      extraMetadataLoading: false,
      extraMetadataError: null,
      extraMetadata: null,
      permissionsLoading: false,
      permissions: null,
    })
  }

  const openModal = async (entries: Entry[]) => {
    const nextToken = ++token
    const files = entries.filter((e) => e.kind === 'file')
    const dirs = entries.filter((e) => e.kind === 'dir')
    const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)
    const fileCount = files.length

    state.set({
      open: true,
      entry: entries.length === 1 ? entries[0] : null,
      targets: entries,
      count: entries.length,
      size: fileBytes,
      itemCount: dirs.length === 0 ? fileCount : null,
      hidden: combine(entries.map((e) => e.hidden == true)),
      extraMetadataLoading: entries.length === 1,
      extraMetadataError: null,
      extraMetadata: null,
      permissionsLoading: true,
      permissions: null,
    })

    if (entries.length === 1) {
      const entry = entries[0]
      void loadPermissions(entry, nextToken)
      void loadEntryTimes(entry, nextToken)
      void loadExtraMetadata(entry, nextToken)
    } else {
      void loadPermissionsMulti(entries, nextToken)
    }

    if (dirs.length > 0) {
      const { total, items } = await computeDirStats(
        dirs.map((d) => d.path),
        (partialBytes) => {
          if (nextToken === token) {
            state.update((s) => ({ ...s, size: fileBytes + partialBytes }))
          }
        },
      )
      if (nextToken === token) {
        state.update((s) => ({ ...s, size: fileBytes + total, itemCount: fileCount + items }))
      }
    } else {
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
              readOnly: null,
              executable: executableSupported ? combine(execVals) : null,
              ownerName: combinePrincipal(ownerNameVals),
              groupName: combinePrincipal(groupNameVals),
              owner: null,
              group: null,
              other: null,
            },
      }))
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
          readOnly: perms.read_only,
          executable: perms.executable,
          ownerName: perms.owner_name ?? null,
          groupName: perms.group_name ?? null,
          owner: perms.owner ?? null,
          group: perms.group ?? null,
          other: perms.other ?? null,
        },
      }))
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
      state.update((s) => ({
        ...s,
        extraMetadataLoading: false,
        extraMetadataError: null,
        extraMetadata: metadata,
      }))
    } catch (err) {
      if (currToken !== token) return
      state.update((s) => ({
        ...s,
        extraMetadataLoading: false,
        extraMetadataError: err instanceof Error ? err.message : String(err),
        extraMetadata: null,
      }))
    }
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
      console.error('Failed to update permissions', { targets, opts, err })
      if (prevState) {
        state.update((s) => ({ ...s, permissions: prevState }))
      }
    }
  }

  const toggleAccess = (scope: 'owner' | 'group' | 'other', key: 'read' | 'write' | 'exec', next: boolean) => {
    const current = get(state)
    const perms = current.permissions
    if (!perms || !perms.accessSupported) return
    const prev = JSON.parse(JSON.stringify(perms)) as PermissionsState
    const updatedScope = perms[scope] ? { ...perms[scope], [key]: next } : perms[scope]
    const updated = { ...perms, [scope]: updatedScope }
    state.update((s) => ({ ...s, permissions: updated }))
    void updatePermissions({ [scope]: { [key]: next } }, prev)
  }

  return {
    state,
    open: openModal,
    close,
    toggleAccess,
    async toggleHidden(next: boolean) {
      const current = get(state)
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

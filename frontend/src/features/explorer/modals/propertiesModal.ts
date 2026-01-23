import { writable, get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'

type Access = { read: boolean; write: boolean; exec: boolean }
export type PermissionsState = {
  accessSupported: boolean
  executableSupported: boolean
  readOnly: boolean | null
  executable: boolean | null
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
  permissions: PermissionsState | null
}

type Deps = {
  computeDirStats: (
    paths: string[],
    onProgress?: (bytes: number, items: number) => void,
  ) => Promise<{ total: number; items: number }>
}

export const createPropertiesModal = (deps: Deps) => {
  const { computeDirStats } = deps
  const state = writable<PropertiesState>({
    open: false,
    entry: null,
    targets: [],
    count: 0,
    size: null,
    itemCount: null,
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
      permissions: null,
    })

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

    if (entries.length === 1) {
      const entry = entries[0]
      void loadPermissions(entry, nextToken)
      void loadEntryTimes(entry, nextToken)
    } else {
      state.update((s) => ({
        ...s,
        permissions: {
          accessSupported: true,
          executableSupported: true,
          readOnly: false,
          executable: false,
          owner: { read: false, write: false, exec: false },
          group: { read: false, write: false, exec: false },
          other: { read: false, write: false, exec: false },
        },
      }))
    }
  }

  const loadPermissions = async (entry: Entry, currToken: number) => {
    try {
      const perms = await invoke<{
        access_supported: boolean
        executable_supported: boolean
        read_only: boolean
        executable: boolean | null
        owner?: Access
        group?: Access
        other?: Access
      }>('get_permissions', { path: entry.path })
      if (currToken !== token) return
      state.update((s) => ({
        ...s,
        permissions: {
          accessSupported: perms.access_supported,
          executableSupported: perms.executable_supported,
          readOnly: perms.read_only,
          executable: perms.executable,
          owner: perms.owner ?? null,
          group: perms.group ?? null,
          other: perms.other ?? null,
        },
      }))
    } catch (err) {
      console.error('Failed to load permissions', err)
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

    try {
      const perms = await invoke<{
        access_supported?: boolean
        executable_supported?: boolean
        read_only?: boolean
        executable?: boolean | null
        owner?: Access
        group?: Access
        other?: Access
      }>('set_permissions', payload)
      const currentPerms = get(state).permissions
      state.update((s) => ({
        ...s,
        permissions: {
          accessSupported: perms.access_supported ?? currentPerms?.accessSupported ?? false,
          executableSupported: perms.executable_supported ?? currentPerms?.executableSupported ?? false,
          readOnly: perms.read_only ?? currentPerms?.readOnly ?? null,
          executable: perms.executable ?? currentPerms?.executable ?? null,
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

  const toggleFlag = (key: 'readOnly' | 'executable', next: boolean) => {
    const current = get(state)
    const perms = current.permissions
    if (!perms) return
    const prev = JSON.parse(JSON.stringify(perms)) as PermissionsState
    state.update((s) => ({
      ...s,
      permissions: { ...perms, [key]: next },
    }))
    void updatePermissions(key === 'readOnly' ? { readOnly: next } : { executable: next }, prev)
  }

  return {
    state,
    open: openModal,
    close,
    toggleAccess,
    toggleFlag,
  }
}

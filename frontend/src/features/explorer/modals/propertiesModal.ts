import { writable, get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'

type AccessBit = boolean | 'mixed'
type Access = { read: AccessBit; write: AccessBit; exec: AccessBit }
export type PermissionsState = {
  accessSupported: boolean
  executableSupported: boolean
  readOnly: AccessBit | null
  executable: AccessBit | null
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
      void loadPermissionsMulti(entries, nextToken)
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

  const loadPermissionsMulti = async (entries: Entry[], currToken: number) => {
    try {
      const sample = entries.slice(0, 50) // limit to avoid huge bursts
      const permsList = await Promise.all(
        sample.map((e) =>
          invoke<{
            access_supported: boolean
            executable_supported: boolean
            read_only: boolean
            executable: boolean | null
            owner?: Access
            group?: Access
            other?: Access
          }>('get_permissions', { path: e.path }),
        ),
      )
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

      state.update((s) => ({
        ...s,
        permissions: accessSupported
          ? {
              accessSupported,
              executableSupported,
              readOnly: combine(readOnlyVals),
              executable: executableSupported ? combine(execVals) : null,
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
              owner: null,
              group: null,
              other: null,
            },
      }))
    } catch (err) {
      console.error('Failed to load multi permissions', err)
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

  return {
    state,
    open: openModal,
    close,
    toggleAccess,
    async toggleHidden(next: boolean) {
      const current = get(state)
      if (!current.entry) return
      const prevPath = current.entry.path
      try {
        const newPath = await invoke<string>('set_hidden', { path: prevPath, hidden: next })
        state.update((s) => ({
          ...s,
          entry: s.entry
            ? {
                ...s.entry,
                path: newPath,
                name: stdPathName(newPath),
                hidden: next,
              }
            : s.entry,
        }))
      } catch (err) {
        console.error('Failed to toggle hidden', err)
      }
    },
  }
}

const stdPathName = (p: string) => {
  const parts = p.split(/[\\/]/)
  return parts[parts.length - 1] ?? p
}

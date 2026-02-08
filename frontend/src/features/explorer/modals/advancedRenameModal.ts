import { get, writable } from 'svelte/store'
import type { Entry } from '../types'
import { renameEntries } from '../services/files'
import { computeAdvancedRenamePreview } from './advancedRenameUtils'

export type SequenceMode = 'none' | 'numeric' | 'alpha'

export type AdvancedRenamePayload = {
  regex: string
  replacement: string
  prefix: string
  suffix: string
  caseSensitive: boolean
  sequenceMode: SequenceMode
  sequenceStart: number
  sequenceStep: number
  sequencePad: number
}

export type AdvancedRenameState = {
  open: boolean
  entries: Entry[]
} & AdvancedRenamePayload & {
  error: string
}

type Deps = {
  reloadCurrent: () => Promise<void>
  showToast: (msg: string, timeout?: number) => void
}

export const createAdvancedRenameModal = ({ reloadCurrent, showToast }: Deps) => {
  const state = writable<AdvancedRenameState>({
    open: false,
    entries: [],
    regex: '',
    replacement: '',
    prefix: '',
    suffix: '',
    caseSensitive: true,
    sequenceMode: 'none',
    sequenceStart: 1,
    sequenceStep: 1,
    sequencePad: 2,
    error: '',
  })

  const open = (entries: Entry[]) => {
    state.set({
      open: true,
      entries,
      regex: '',
      replacement: '',
      prefix: '',
      suffix: '',
      caseSensitive: true,
      sequenceMode: 'none',
      sequenceStart: 1,
      sequenceStep: 1,
      sequencePad: 2,
      error: '',
    })
  }

  const close = () => {
    state.update((s) => ({ ...s, open: false, entries: [], error: '' }))
  }

  const confirm = async () => {
    const current = get(state)
    if (!current.open || current.entries.length === 0) return false

    const { rows, error } = computeAdvancedRenamePreview(current.entries, current)
    if (error) {
      state.update((s) => ({ ...s, error }))
      return false
    }

    const entries = current.entries.map((entry, idx) => ({
      path: entry.path,
      newName: rows[idx]?.next ?? entry.name,
    }))

    try {
      const renamed = await renameEntries(entries)
      await reloadCurrent()
      close()
      if (renamed.length === 0) {
        showToast('No names changed')
      } else {
        showToast(`Renamed ${renamed.length} item${renamed.length === 1 ? '' : 's'}`)
      }
      return true
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      state.update((s) => ({ ...s, error: msg }))
      return false
    }
  }

  return { state, open, close, confirm }
}

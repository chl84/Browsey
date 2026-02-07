import { writable } from 'svelte/store'
import type { Entry } from '../types'

export type SequenceMode = 'numeric' | 'alpha'

export type AdvancedRenameState = {
  open: boolean
  entries: Entry[]
  regex: string
  replacement: string
  caseSensitive: boolean
  sequenceMode: SequenceMode
  sequenceStart: number
  sequenceStep: number
  sequencePad: number
  error: string
}

export const createAdvancedRenameModal = () => {
  const state = writable<AdvancedRenameState>({
    open: false,
    entries: [],
    regex: '',
    replacement: '',
    caseSensitive: true,
    sequenceMode: 'numeric',
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
      caseSensitive: true,
      sequenceMode: 'numeric',
      sequenceStart: 1,
      sequenceStep: 1,
      sequencePad: 2,
      error: '',
    })
  }

  const close = () => {
    state.update((s) => ({ ...s, open: false, entries: [], error: '' }))
  }

  // Placeholder confirm hook; real rename logic will be added later.
  const confirm = () => {
    state.update((s) => ({ ...s, error: '' }))
  }

  return { state, open, close, confirm }
}

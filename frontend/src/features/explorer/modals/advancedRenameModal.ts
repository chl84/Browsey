import { get, writable } from 'svelte/store'
import type { Entry } from '../model/types'
import {
  previewRenameEntries,
  renameEntries,
  type AdvancedRenamePreviewRow,
} from '../services/files.service'

export type SequenceMode = 'none' | 'numeric' | 'alpha'
export type SequencePlacement = 'start' | 'end'

export type AdvancedRenamePayload = {
  regex: string
  replacement: string
  prefix: string
  suffix: string
  caseSensitive: boolean
  sequenceMode: SequenceMode
  sequencePlacement: SequencePlacement
  sequenceStart: number
  sequenceStep: number
  sequencePad: number
}

export type AdvancedRenameState = {
  open: boolean
  entries: Entry[]
} & AdvancedRenamePayload & {
  error: string
  preview: AdvancedRenamePreviewRow[]
  previewError: string
  previewLoading: boolean
}

type Deps = {
  reloadCurrent: () => Promise<void>
  showToast: (msg: string, timeout?: number) => void
}

export const createAdvancedRenameModal = ({ reloadCurrent, showToast }: Deps) => {
  const defaultPayload = (): AdvancedRenamePayload => ({
    regex: '',
    replacement: '',
    prefix: '',
    suffix: '',
    caseSensitive: true,
    sequenceMode: 'none',
    sequencePlacement: 'end',
    sequenceStart: 1,
    sequenceStep: 1,
    sequencePad: 2,
  })

  const normalizeInt = (value: number, fallback: number): number =>
    Number.isFinite(value) ? Math.round(value) : fallback

  const normalizePayload = (payload: AdvancedRenamePayload): AdvancedRenamePayload => ({
    ...payload,
    sequenceStart: normalizeInt(payload.sequenceStart, 1),
    sequenceStep: normalizeInt(payload.sequenceStep, 1),
    sequencePad: Math.max(0, normalizeInt(payload.sequencePad, 2)),
  })

  const previewInputs = (entries: Entry[]) => entries.map((entry) => ({ path: entry.path, name: entry.name }))

  const invokeErrorMessage = (err: unknown): string => {
    if (err instanceof Error && err.message.trim().length > 0) return err.message
    if (typeof err === 'string' && err.trim().length > 0) {
      const raw = err.trim()
      try {
        const parsed = JSON.parse(raw) as Record<string, unknown>
        if (typeof parsed.message === 'string' && parsed.message.trim().length > 0) {
          return parsed.message
        }
      } catch {
        // Keep raw string fallback below.
      }
      return raw
    }
    if (err && typeof err === 'object') {
      const record = err as Record<string, unknown>
      if (typeof record.message === 'string' && record.message.trim().length > 0) {
        return record.message
      }
      if (record.error && typeof record.error === 'object') {
        const nested = record.error as Record<string, unknown>
        if (typeof nested.message === 'string' && nested.message.trim().length > 0) {
          return nested.message
        }
      }
    }
    return 'Unknown error'
  }

  const state = writable<AdvancedRenameState>({
    open: false,
    entries: [],
    ...defaultPayload(),
    error: '',
    preview: [],
    previewError: '',
    previewLoading: false,
  })
  let previewToken = 0

  const updatePreviewState = (token: number, rows: AdvancedRenamePreviewRow[], previewError = '') => {
    if (token !== previewToken) return
    state.update((s) => ({
      ...s,
      preview: rows,
      previewError,
      previewLoading: false,
    }))
  }

  const requestPreview = async (
    entries: Entry[],
    payload: AdvancedRenamePayload,
  ): Promise<{ rows: AdvancedRenamePreviewRow[]; error: string }> => {
    try {
      const result = await previewRenameEntries(previewInputs(entries), payload)
      return {
        rows: Array.isArray(result.rows) ? result.rows : [],
        error: typeof result.error === 'string' ? result.error : '',
      }
    } catch (err) {
      return {
        rows: entries.map((entry) => ({ original: entry.name, next: entry.name })),
        error: invokeErrorMessage(err),
      }
    }
  }

  const refreshPreview = async (token: number, entries: Entry[], payload: AdvancedRenamePayload) => {
    const preview = await requestPreview(entries, payload)
    updatePreviewState(token, preview.rows, preview.error)
  }

  const change = (payload: AdvancedRenamePayload) => {
    const normalized = normalizePayload(payload)
    const current = get(state)
    if (!current.open) return
    state.update((s) => ({
      ...s,
      ...normalized,
      error: '',
      previewLoading: true,
      previewError: '',
    }))
    const updated = get(state)
    const token = ++previewToken
    void refreshPreview(token, updated.entries, normalizePayload(updated))
  }

  const open = (entries: Entry[]) => {
    const payload = defaultPayload()
    state.set({
      open: true,
      entries,
      ...payload,
      error: '',
      preview: entries.map((entry) => ({ original: entry.name, next: entry.name })),
      previewError: '',
      previewLoading: true,
    })
    const token = ++previewToken
    void refreshPreview(token, entries, payload)
  }

  const close = () => {
    previewToken += 1
    state.update((s) => ({
      ...s,
      open: false,
      entries: [],
      error: '',
      preview: [],
      previewError: '',
      previewLoading: false,
    }))
  }

  const confirm = async () => {
    const current = get(state)
    if (!current.open || current.entries.length === 0) return false

    const normalized = normalizePayload(current)
    state.update((s) => ({ ...s, previewLoading: true, previewError: '', error: '' }))
    const token = ++previewToken
    const { rows, error } = await requestPreview(current.entries, normalized)
    if (token !== previewToken) return false
    updatePreviewState(token, rows, error)
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
      const msg = invokeErrorMessage(err)
      state.update((s) => ({ ...s, error: msg }))
      return false
    }
  }

  return { state, open, close, change, confirm }
}

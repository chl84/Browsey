import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import type { OpenWithApp, OpenWithChoice } from '../services/openWith'
import { fetchOpenWithApps, openWithSelection, defaultOpenWithApp } from '../services/openWith'

export type OpenWithState = {
  open: boolean
  entry: Entry | null
  apps: OpenWithApp[]
  loading: boolean
  error: string
  submitting: boolean
}

type Deps = {
  showToast: (msg: string) => void
}

export const createOpenWithModal = (deps: Deps) => {
  const { showToast } = deps
  const state = writable<OpenWithState>({
    open: false,
    entry: null,
    apps: [],
    loading: false,
    error: '',
    submitting: false,
  })
  let loadId = 0

  const close = () => {
    state.set({
      open: false,
      entry: null,
      apps: [],
      loading: false,
      error: '',
      submitting: false,
    })
  }

  const loadOpenWithApps = async (path: string) => {
    const requestId = ++loadId
    state.update((s) => ({ ...s, loading: true, error: '', apps: [defaultOpenWithApp] }))
    try {
      const list = await fetchOpenWithApps(path)
      const curr = get(state)
      if (!curr.open || curr.entry?.path !== path || requestId !== loadId) return
      state.update((s) => ({ ...s, apps: [defaultOpenWithApp, ...list] }))
    } catch (err) {
      const curr = get(state)
      if (!curr.open || curr.entry?.path !== path || requestId !== loadId) return
      state.update((s) => ({
        ...s,
        apps: [defaultOpenWithApp],
        error: getErrorMessage(err),
      }))
    } finally {
      if (requestId === loadId) {
        state.update((s) => ({ ...s, loading: false }))
      }
    }
  }

  const open = (entry: Entry) => {
    state.set({
      open: true,
      entry,
      apps: [defaultOpenWithApp],
      loading: false,
      error: '',
      submitting: false,
    })
    void loadOpenWithApps(entry.path)
  }

  const confirm = async (choice: OpenWithChoice) => {
    const current = get(state)
    if (!current.open || !current.entry || current.submitting) return
    const normalized: OpenWithChoice = {
      appId: choice.appId ?? undefined,
    }
    const hasApp = Boolean(normalized.appId)
    if (!hasApp) {
      state.update((s) => ({ ...s, error: 'Pick an application.' }))
      return
    }
    state.update((s) => ({ ...s, submitting: true, error: '' }))
    try {
      await openWithSelection(current.entry.path, normalized)
      showToast(`Opening ${current.entry.name}…`)
      close()
    } catch (err) {
      state.update((s) => ({
        ...s,
        error: getErrorMessage(err),
      }))
    } finally {
      state.update((s) => ({ ...s, submitting: false }))
    }
  }

  return {
    state,
    open,
    close,
    confirm,
  }
}

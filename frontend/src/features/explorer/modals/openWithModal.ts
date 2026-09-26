import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import type { OpenWithApp, OpenWithChoice } from '../services/openWith.service'
import { fetchOpenWithApps, openWithSelection, setDefaultApplication, defaultOpenWithApp } from '../services/openWith.service'

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
    if (get(state).submitting) return
    ++loadId
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
    if (get(state).submitting) return
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
    if (!current.open || !current.entry || current.submitting || current.loading) return
    const normalized: OpenWithChoice = {
      appId: choice.appId ?? undefined,
    }
    const app = current.apps.find((app) => app.id === normalized.appId)
    if (!app) {
      state.update((s) => ({ ...s, error: 'Pick an application.' }))
      return
    }
    if (choice.setDefault && !app.defaultContentType) {
      state.update((s) => ({ ...s, error: 'A default cannot be set for this application or file type.' }))
      return
    }
    state.update((s) => ({ ...s, submitting: true, error: '' }))
    let defaultSaved = false
    try {
      if (choice.setDefault && app.defaultContentType) {
        await setDefaultApplication(current.entry.path, app.id, app.defaultContentType)
        defaultSaved = true
      }
      await openWithSelection(current.entry.path, normalized)
      showToast(defaultSaved
        ? `Opening ${current.entry.name}… ${app.name} is now the default for ${app.defaultContentType}`
        : `Opening ${current.entry.name}…`)
      state.update((s) => ({ ...s, submitting: false }))
      close()
    } catch (err) {
      state.update((s) => ({
        ...s,
        error: defaultSaved
          ? `The default application was saved, but the file could not be opened: ${getErrorMessage(err)}`
          : getErrorMessage(err),
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

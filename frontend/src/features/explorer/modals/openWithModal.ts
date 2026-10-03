import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { prepareCloudWorkingCopy } from '@/features/network'
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
  let localPath: string | null = null

  const close = () => {
    if (get(state).submitting) return
    ++loadId
    localPath = null
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
      const target = path.startsWith('rclone://') ? (await prepareCloudWorkingCopy(path)).localPath : path
      if (requestId !== loadId) return
      localPath = target
      const list = await fetchOpenWithApps(target)
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
    localPath = null
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
    if (!localPath) return
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
        await setDefaultApplication(localPath, app.id, app.defaultContentType)
        defaultSaved = true
      }
      await openWithSelection(localPath, normalized)
      const message = defaultSaved
        ? `Opening ${current.entry.name}… ${app.name} is now the default for ${app.defaultContentType}`
        : `Opening ${current.entry.name}…`
      showToast(current.entry.path.startsWith('rclone://')
        ? `${message} You are editing a local working copy. Upload edits from Settings → Cloud → Working copies.`
        : message)
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

import { writable, get } from 'svelte/store'
import { createActivity, type ActivityState } from '../hooks/createActivity'
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
  progress: ActivityState | null
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
    progress: null,
  })
  let loadId = 0
  let localPath: string | null = null
  let cancelPreparation: (() => void) | null = null

  const close = () => {
    if (get(state).submitting) return
    cancelPreparation?.()
    cancelPreparation = null
    ++loadId
    localPath = null
    state.set({
      open: false,
      entry: null,
      apps: [],
      loading: false,
      error: '',
      submitting: false,
      progress: null,
    })
  }

  const loadOpenWithApps = async (path: string) => {
    const requestId = ++loadId
    const cloud = path.startsWith('rclone://')
    const progressApi = createActivity()
    const event = `cloud-open-with-${Date.now()}-${Math.random().toString(16).slice(2)}`
    const cancel = () => void progressApi.requestCancel(event)
    if (cloud) cancelPreparation = cancel
    const unsubscribe = progressApi.activity.subscribe(progress => {
      if (requestId === loadId) state.update(s => ({ ...s, progress }))
    })
    state.update((s) => ({ ...s, loading: true, error: '', apps: [defaultOpenWithApp] }))
    try {
      if (cloud) await progressApi.start('Downloading cloud file…', event, cancel, { completeOnReply: true })
      if (requestId !== loadId) return
      const target = cloud ? (await prepareCloudWorkingCopy(path, event)).localPath : path
      progressApi.clearNow()
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
      unsubscribe()
      await progressApi.cleanup()
      if (cancelPreparation === cancel) cancelPreparation = null
      if (requestId === loadId) {
        state.update((s) => ({ ...s, loading: false, progress: null }))
      }
    }
  }

  const open = (entry: Entry) => {
    if (get(state).submitting) return
    cancelPreparation?.()
    cancelPreparation = null
    localPath = null
    state.set({
      open: true,
      entry,
      apps: [defaultOpenWithApp],
      loading: false,
      error: '',
      submitting: false,
      progress: null,
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
        ? `${message} Cloud saves shows upload status and any conflicts.`
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

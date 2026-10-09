import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { get, writable } from 'svelte/store'
import { getErrorMessage, normalizeError } from '@/shared/lib/error'
import { cancelTask } from '../services/activity.service'
import { progressPresentation, type ProgressPayload } from './progress'

export type ActivityState = {
  label: string
  detail?: string | null
  percent: number | null
  cancel?: (() => void) | null
  cancelling?: boolean
}

export type ActivityApi = Omit<ReturnType<typeof createActivity>, 'activity'>

export type { ProgressPayload } from './progress'

type Options = {
  onError?: (message: string) => void
}

export const createActivity = (opts: Options = {}) => {
  const { onError } = opts
  const activity = writable<ActivityState | null>(null)

  let activityHideTimer: ReturnType<typeof setTimeout> | null = null
  let activityUnlisten: UnlistenFn | null = null
  let activityGeneration = 0
  let progressReporter: { eventName: string; report: (payload: ProgressPayload) => void } | null = null

  const queueActivityHide = () => {
    if (activityHideTimer) {
      clearTimeout(activityHideTimer)
    }
    activityHideTimer = setTimeout(() => {
      activity.set(null)
      activityHideTimer = null
    }, 1200)
  }

  const hasHideTimer = () => activityHideTimer !== null

  const cleanup = async (preserveTimer = false) => {
    activityGeneration += 1
    progressReporter = null
    if (activityUnlisten) {
      await activityUnlisten()
      activityUnlisten = null
    }
    if (!preserveTimer && activityHideTimer) {
      clearTimeout(activityHideTimer)
      activityHideTimer = null
    }
  }

  const clearNow = () => {
    activity.set(null)
    if (activityHideTimer) {
      clearTimeout(activityHideTimer)
      activityHideTimer = null
    }
  }

  const start = async (label: string, eventName: string, onCancel?: () => void,
    options?: { completeOnReply?: boolean }) => {
    await cleanup()
    if (activityHideTimer) {
      clearTimeout(activityHideTimer)
      activityHideTimer = null
    }
    activity.set({ label, detail: null, percent: null, cancel: onCancel ?? null, cancelling: false })
    let phaseLabel = label
    const generation = activityGeneration
    const report = (payload: ProgressPayload) => {
      if (generation !== activityGeneration) return
      if (payload.phase) phaseLabel = payload.phase
      const { percent: pct, detail } = progressPresentation(payload)
      const existing = get(activity)
      const cancelling = existing?.cancelling ?? false
      const displayLabel = cancelling ? 'Cancelling…' : phaseLabel
      if (payload.finished && options?.completeOnReply) {
        // A staged operation has more phases after a child transfer/archive
        // finishes. Only the command reply owns final completion and cleanup.
        activity.set({ label: displayLabel, detail: null, percent: null,
          cancel: cancelling ? null : existing?.cancel ?? onCancel ?? null, cancelling })
      } else if (payload.finished) {
        activity.set({
          label: cancelling ? 'Cancelling…' : 'Finalizing…',
          detail,
          percent: pct ?? null,
          cancel: null,
          cancelling,
        })
        queueActivityHide()
      } else {
        activity.set({
          label: displayLabel,
          detail: cancelling ? null : detail,
          percent: pct,
          cancel: cancelling ? null : existing?.cancel ?? onCancel ?? null,
          cancelling,
        })
      }
    }
    progressReporter = { eventName, report }
    activityUnlisten = await listen<ProgressPayload>(eventName, (event) => report(event.payload))
  }

  const reportProgress = (eventName: string, payload: ProgressPayload) => {
    if (progressReporter?.eventName === eventName) progressReporter.report(payload)
  }

  const requestCancel = async (eventName: string) => {
    const current = get(activity)
    if (!current || current.cancelling) return
    activity.set({ ...current, label: 'Cancelling…', cancel: null, cancelling: true })
    try {
      await cancelTask(eventName)
    } catch (err) {
      // The caller also records cancellation while preparing metadata or
      // between roots. A task may not exist yet, or its reply may be pending.
      // Keep the operation visible until that caller acknowledges completion.
      if (normalizeError(err).code === 'task_not_found') return
      const msg = getErrorMessage(err)
      onError?.(`Cancel failed: ${msg}`)
      clearNow()
      await cleanup()
    }
  }

  return {
    activity,
    start,
    requestCancel,
    reportProgress,
    cleanup,
    clearNow,
    hasHideTimer,
    hideSoon: queueActivityHide,
  }
}

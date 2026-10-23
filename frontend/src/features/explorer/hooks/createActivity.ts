import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { writable } from 'svelte/store'
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

  type Active = {
    eventName: string
    state: ActivityState | null
    active: boolean
    unlisten: UnlistenFn | null
    hideTimer: ReturnType<typeof setTimeout> | null
    report: ((payload: ProgressPayload) => void) | null
  }
  const activities = new Map<string, Active>()
  const visible = () => [...activities.values()].reverse().find(item => item.state !== null)
  const resolve = (eventName?: string) => eventName === undefined ? visible() : activities.get(eventName)
  const publish = () => activity.set(visible()?.state ?? null)

  const cleanup = async (preserveTimer = false, eventName?: string) => {
    const item = resolve(eventName)
    if (!item) return
    item.active = false
    item.report = null
    const unlisten = item.unlisten
    item.unlisten = null
    if (preserveTimer && item.state) item.state = { ...item.state, cancel: null }
    if (!preserveTimer) {
      if (item.hideTimer) clearTimeout(item.hideTimer)
      item.hideTimer = null
      item.state = null
    }
    if (!preserveTimer || item.state === null) activities.delete(item.eventName)
    publish()
    await unlisten?.()
  }

  const clearNow = (eventName?: string) => {
    const item = resolve(eventName)
    if (!item) return
    item.state = null
    if (item.hideTimer) clearTimeout(item.hideTimer)
    item.hideTimer = null
    if (!item.active) activities.delete(item.eventName)
    publish()
  }

  const queueActivityHide = (eventName?: string) => {
    const item = resolve(eventName)
    if (!item) return
    if (item.hideTimer) clearTimeout(item.hideTimer)
    item.hideTimer = setTimeout(() => {
      if (activities.get(item.eventName) !== item) return
      item.hideTimer = null
      clearNow(item.eventName)
      void cleanup(false, item.eventName).catch(() => {})
    }, 1200)
  }
  const hasHideTimer = (eventName?: string) => resolve(eventName)?.hideTimer != null

  const start = async (label: string, eventName: string, onCancel?: () => void,
    options?: { completeOnReply?: boolean }) => {
    // Restarting one operation replaces only its own registration. Other
    // operations keep their state/listener and reappear when newer work ends.
    const previous = activities.get(eventName)
    const releasing = previous ? cleanup(false, eventName) : Promise.resolve()
    const item: Active = {
      eventName, state: { label, detail: null, percent: null, cancel: onCancel ?? null, cancelling: false },
      active: true, unlisten: null, hideTimer: null, report: null,
    }
    activities.delete(eventName)
    activities.set(eventName,item)
    publish()
    let phaseLabel = label
    const report = (payload: ProgressPayload) => {
      if (activities.get(eventName) !== item || !item.active || !item.state) return
      if (payload.phase) phaseLabel = payload.phase
      const { percent: pct, detail } = progressPresentation(payload)
      const existing = item.state
      const cancelling = existing.cancelling ?? false
      const displayLabel = cancelling ? 'Cancelling…' : phaseLabel
      if (payload.finished && options?.completeOnReply) {
        item.state = { label: displayLabel, detail: null, percent: null,
          cancel: cancelling ? null : existing.cancel ?? onCancel ?? null, cancelling }
      } else if (payload.finished) {
        item.state = { label: cancelling ? 'Cancelling…' : 'Finalizing…',
          detail, percent: pct ?? null, cancel: null, cancelling }
        queueActivityHide(eventName)
      } else {
        item.state = { label: displayLabel, detail: cancelling ? null : detail,
          percent: pct, cancel: cancelling ? null : existing.cancel ?? onCancel ?? null, cancelling }
      }
      publish()
    }
    item.report = report
    try {
      await releasing
      if (activities.get(eventName) !== item || !item.active) return
      const unlisten = await listen<ProgressPayload>(eventName, event => report(event.payload))
      if (activities.get(eventName) !== item || !item.active) {
        await unlisten()
        return
      }
      item.unlisten = unlisten
    } catch (error) {
      if (activities.get(eventName) === item) {
        clearNow(eventName)
        await cleanup(false,eventName)
      }
      throw error
    }
  }

  const reportProgress = (eventName: string, payload: ProgressPayload) => {
    activities.get(eventName)?.report?.(payload)
  }

  const requestCancel = async (eventName: string) => {
    const item = activities.get(eventName)
    if (!item?.state || !item.active || item.state.cancelling) return
    const previous = item.state
    item.state = { ...previous, label: 'Cancelling…', cancel: null, cancelling: true }
    publish()
    try {
      await cancelTask(eventName)
    } catch (err) {
      if (activities.get(eventName) !== item || !item.active) return
      if (normalizeError(err).code === 'task_not_found') return
      if (visible() !== item) {
        // Retain a background operation's controls so cancellation can be
        // retried when it becomes visible, without changing newer feedback.
        item.state = previous
        publish()
        return
      }
      onError?.(`Cancel failed: ${getErrorMessage(err)}`)
      clearNow(eventName)
      await cleanup(false,eventName)
    }
  }

  const cleanupAll = async () => {
    const all = [...activities.values()]
    activities.clear()
    activity.set(null)
    await Promise.allSettled(all.map(async item => {
      item.active = false
      if (item.hideTimer) clearTimeout(item.hideTimer)
      await item.unlisten?.()
    }))
  }

  return { activity, start, requestCancel, reportProgress, cleanup, cleanupAll,
    clearNow, hasHideTimer, hideSoon: queueActivityHide }
}

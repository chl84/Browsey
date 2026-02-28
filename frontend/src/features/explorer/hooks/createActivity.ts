import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { get, writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { cancelTask } from '../services/activity.service'

export type ActivityState = {
  label: string
  detail?: string | null
  percent: number | null
  cancel?: (() => void) | null
  cancelling?: boolean
}

export type ProgressPayload = { bytes: number; total: number; finished?: boolean }

type Options = {
  onError?: (message: string) => void
}

const formatByteProgress = (bytes: number, total: number) => {
  const formatSize = (value: number) => {
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    let size = value
    let unitIndex = 0
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024
      unitIndex += 1
    }
    const precision = unitIndex === 0 ? 0 : size >= 100 ? 0 : size >= 10 ? 1 : 2
    return `${size.toFixed(precision)} ${units[unitIndex]}`
  }

  return `${formatSize(bytes)} / ${formatSize(total)}`
}

export const createActivity = (opts: Options = {}) => {
  const { onError } = opts
  const activity = writable<ActivityState | null>(null)

  let activityHideTimer: ReturnType<typeof setTimeout> | null = null
  let activityUnlisten: UnlistenFn | null = null

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

  const start = async (label: string, eventName: string, onCancel?: () => void) => {
    await cleanup()
    if (activityHideTimer) {
      clearTimeout(activityHideTimer)
      activityHideTimer = null
    }
    activity.set({ label, detail: null, percent: null, cancel: onCancel ?? null, cancelling: false })
    activityUnlisten = await listen<ProgressPayload>(eventName, (event) => {
      const payload = event.payload
      let pct =
        payload.total > 0 ? Math.min(100, Math.round((payload.bytes / payload.total) * 100)) : null
      if (pct === 0 && payload.bytes > 0) {
        pct = 1
      }
      const existing = get(activity)
      const cancelling = existing?.cancelling ?? false
      const displayLabel = cancelling ? 'Cancelling…' : label
      const detail = payload.total > 0 ? formatByteProgress(payload.bytes, payload.total) : null
      if (payload.finished) {
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
    })
  }

  const requestCancel = async (eventName: string) => {
    const current = get(activity)
    if (!current || current.cancelling) return
    activity.set({ ...current, label: 'Cancelling…', cancel: null, cancelling: true })
    try {
      await cancelTask(eventName)
    } catch (err) {
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
    cleanup,
    clearNow,
    hasHideTimer,
    hideSoon: queueActivityHide,
  }
}

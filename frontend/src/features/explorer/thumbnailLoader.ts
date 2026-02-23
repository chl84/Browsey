import { invoke } from '@/shared/lib/tauri'
import { getErrorMessage } from '@/shared/lib/error'
import { writable, type Readable } from 'svelte/store'

type Options = {
  maxConcurrent?: number
  maxConcurrentVideos?: number
  maxDim?: number
  initialGeneration?: string
  allowVideos?: boolean
}

type ThumbMap = Map<string, string>

const DEFAULT_CONCURRENCY = 4
const DEFAULT_VIDEO_CONCURRENCY = 1
const DEFAULT_DIM = 96
const MAX_RETRIES = 3
const BASE_BACKOFF_MS = 300
type Priority = 'high' | 'low'

export function createThumbnailLoader(opts: Options = {}) {
  const maxConcurrent = opts.maxConcurrent ?? DEFAULT_CONCURRENCY
  const maxConcurrentVideos = Math.max(0, Math.min(opts.maxConcurrentVideos ?? DEFAULT_VIDEO_CONCURRENCY, maxConcurrent))
  const maxDim = opts.maxDim ?? DEFAULT_DIM
  let generation = opts.initialGeneration ?? 'init'
  let allowVideos = opts.allowVideos ?? true

  const thumbs = writable<ThumbMap>(new Map())
  const requested = new Set<string>()
  const highQueue: string[] = []
  const lowQueue: string[] = []
  const retryTimers = new Map<string, ReturnType<typeof setTimeout>>()
  let active = 0
  let activeVideos = 0
  let destroyed = false
  const retries = new Map<string, number>()
  const videoExt = new Set(['mp4', 'mov', 'm4v', 'webm', 'mkv', 'avi'])
  const isVideo = (path: string) => {
    const ext = path.split('.').pop()?.toLowerCase()
    return ext ? videoExt.has(ext) : false
  }

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const path = observed.get(entry.target)
          if (path) {
            const queued = enqueue(path)
            if (queued) {
              observer.unobserve(entry.target)
              observed.delete(entry.target)
            }
          }
        }
      }
    },
    { root: null, rootMargin: '200px 0px', threshold: 0.01 }
  )

  const observed = new Map<Element, string>()

  function enqueue(path: string, priority?: Priority) {
    if (destroyed) return false
    if (!allowVideos && isVideo(path)) return false
    clearRetryTimer(path)
    if (requested.has(path)) return false
    requested.add(path)
    const prio = priority ?? (isVideo(path) ? 'low' : 'high')
    if (prio === 'high') {
      highQueue.push(path)
    } else {
      lowQueue.push(path)
    }
    pump()
    return true
  }

  function pump() {
    if (destroyed) return
    while (active < maxConcurrent && (highQueue.length > 0 || lowQueue.length > 0)) {
      const path = dequeueNextEligible()
      if (!path) break
      const genAtStart = generation
      const isVideoTask = isVideo(path)
      active++
      if (isVideoTask) activeVideos++
      loadThumb(path, genAtStart)
        .then((thumbPath) => {
          if (destroyed) return
          if (genAtStart !== generation) return
          if (!thumbPath) return
          thumbs.update((m) => {
            const next = new Map(m)
            next.set(path, thumbPath)
            return next
          })
          retries.delete(path)
          requested.delete(path)
        })
        .catch(() => {
          requested.delete(path)
        })
        .finally(() => {
          active--
          if (isVideoTask) activeVideos = Math.max(0, activeVideos - 1)
          pump()
        })
    }
  }

  function dequeueNextEligible(): string | undefined {
    const high = highQueue.shift()
    if (high) return high
    if (lowQueue.length === 0) return undefined
    if (maxConcurrentVideos < 1 || activeVideos >= maxConcurrentVideos) {
      const nonVideoIndex = lowQueue.findIndex((p) => !isVideo(p))
      if (nonVideoIndex >= 0) {
        const [path] = lowQueue.splice(nonVideoIndex, 1)
        return path
      }
      return undefined
    }
    return lowQueue.shift()
  }

  async function loadThumb(path: string, genAtStart: string): Promise<string | null> {
    try {
      const res = await invoke<{
        path: string
        width: number
        height: number
        cached: boolean
      }>('get_thumbnail', { path, max_dim: maxDim, generation })
      if (genAtStart !== generation) return null
      return res.path
    } catch (err) {
      if (genAtStart !== generation) {
        retries.delete(path)
        return null
      }
      const msg = getErrorMessage(err)

      const isBusy = msg.toLowerCase().includes('too many concurrent thumbnails')
      if (isBusy) {
        const attempt = (retries.get(path) ?? 0) + 1
        if (attempt <= MAX_RETRIES) {
          retries.set(path, attempt)
          const delay = Math.min(BASE_BACKOFF_MS * Math.pow(2, attempt - 1), 2000)
          requested.delete(path)
          const retryId = setTimeout(() => {
            retryTimers.delete(path)
            enqueue(path)
          }, delay)
          retryTimers.set(path, retryId)
        } else {
          retries.delete(path)
          requested.delete(path)
        }
      } else {
        retries.delete(path)
        requested.delete(path)
      }
      return null
    }
  }

  function observe(node: Element, path: string) {
    if (destroyed) {
      return {
        update() {},
        destroy() {},
      }
    }
    observed.set(node, path)
    observer.observe(node)
    return {
      update(newPath: string) {
        observed.set(node, newPath)
      },
      destroy() {
        observer.unobserve(node)
        observed.delete(node)
      },
    }
  }

  function clearRetryTimer(path: string) {
    const retryId = retryTimers.get(path)
    if (retryId !== undefined) {
      clearTimeout(retryId)
      retryTimers.delete(path)
    }
  }

  function clearRetryTimers() {
    retryTimers.forEach((retryId) => clearTimeout(retryId))
    retryTimers.clear()
  }

  return {
    observe,
    reset: (token?: string) => {
      generation = token ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
      requested.clear()
      retries.clear()
      clearRetryTimers()
      highQueue.length = 0
      lowQueue.length = 0
      thumbs.set(new Map())
    },
    setAllowVideos: (value: boolean) => {
      const wasAllowed = allowVideos
      allowVideos = value
      if (!allowVideos) {
        // Drop queued and cached video thumbnails
        for (let i = highQueue.length - 1; i >= 0; i--) {
          if (isVideo(highQueue[i])) highQueue.splice(i, 1)
        }
        for (let i = lowQueue.length - 1; i >= 0; i--) {
          if (isVideo(lowQueue[i])) lowQueue.splice(i, 1)
        }
        requested.forEach((p) => {
          if (isVideo(p)) {
            requested.delete(p)
            clearRetryTimer(p)
          }
        })
        retries.forEach((_, p) => {
          if (isVideo(p)) {
            retries.delete(p)
            clearRetryTimer(p)
          }
        })
        thumbs.update((m) => {
          const next = new Map(m)
          for (const [p] of next) {
            if (isVideo(p)) next.delete(p)
          }
          return next
        })
      } else if (!wasAllowed && allowVideos) {
        const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0
        const margin = 200
        observed.forEach((path, node) => {
          if (!isVideo(path)) return
          const rect = node.getBoundingClientRect()
          const inView = rect.bottom >= -margin && rect.top <= viewportHeight + margin
          if (inView) {
            const queued = enqueue(path, 'low')
            if (queued) {
              observer.unobserve(node)
              observed.delete(node)
            }
          }
        })
      }
    },
    drop: (path: string) => {
      requested.delete(path)
      retries.delete(path)
      clearRetryTimer(path)
      for (let i = highQueue.length - 1; i >= 0; i--) {
        if (highQueue[i] === path) highQueue.splice(i, 1)
      }
      for (let i = lowQueue.length - 1; i >= 0; i--) {
        if (lowQueue[i] === path) lowQueue.splice(i, 1)
      }
      thumbs.update((m) => {
        const next = new Map(m)
        next.delete(path)
        return next
      })
    },
    destroy: () => {
      if (destroyed) return
      destroyed = true
      observer.disconnect()
      observed.clear()
      clearRetryTimers()
      requested.clear()
      retries.clear()
      highQueue.length = 0
      lowQueue.length = 0
      thumbs.set(new Map())
    },
    subscribe: thumbs.subscribe as Readable<ThumbMap>['subscribe'],
  }
}

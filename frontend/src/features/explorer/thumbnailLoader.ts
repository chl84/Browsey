import { invoke } from '@tauri-apps/api/core'
import { writable, type Readable } from 'svelte/store'

type Options = {
  maxConcurrent?: number
  maxDim?: number
  initialGeneration?: string
  allowVideos?: boolean
}

type ThumbMap = Map<string, string>

const DEFAULT_CONCURRENCY = 2
const DEFAULT_DIM = 96
const MAX_RETRIES = 3
const BASE_BACKOFF_MS = 300
type Priority = 'high' | 'low'

type ObsEntry = {
  node: Element
  path: string
}

export function createThumbnailLoader(opts: Options = {}) {
  const maxConcurrent = opts.maxConcurrent ?? DEFAULT_CONCURRENCY
  const maxDim = opts.maxDim ?? DEFAULT_DIM
  let generation = opts.initialGeneration ?? 'init'
  let allowVideos = opts.allowVideos ?? true

  const thumbs = writable<ThumbMap>(new Map())
  const requested = new Set<string>()
  const highQueue: string[] = []
  const lowQueue: string[] = []
  let active = 0
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
    if (!allowVideos && isVideo(path)) return false
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
    while (active < maxConcurrent && (highQueue.length > 0 || lowQueue.length > 0)) {
      const path = highQueue.shift() ?? lowQueue.shift()
      if (!path) break
      const genAtStart = generation
      active++
      loadThumb(path, genAtStart)
        .then((thumbPath) => {
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
          pump()
        })
    }
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
      const msg =
        typeof err === 'string'
          ? err
          : err && typeof err === 'object' && 'message' in err
            ? String((err as { message?: unknown }).message)
            : String(err)

      const isBusy = msg.toLowerCase().includes('too many concurrent thumbnails')
      if (isBusy) {
        const attempt = (retries.get(path) ?? 0) + 1
        if (attempt <= MAX_RETRIES) {
          retries.set(path, attempt)
          const delay = Math.min(BASE_BACKOFF_MS * Math.pow(2, attempt - 1), 2000)
          requested.delete(path)
          setTimeout(() => enqueue(path), delay)
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

  return {
    observe,
    reset: (token?: string) => {
      generation = token ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
      requested.clear()
      retries.clear()
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
          if (isVideo(p)) requested.delete(p)
        })
        retries.forEach((_, p) => {
          if (isVideo(p)) retries.delete(p)
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
    subscribe: thumbs.subscribe as Readable<ThumbMap>['subscribe'],
  }
}

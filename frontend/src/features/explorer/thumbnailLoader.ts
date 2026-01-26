import { invoke } from '@tauri-apps/api/core'
import { writable, type Readable } from 'svelte/store'

type Options = {
  maxConcurrent?: number
  maxDim?: number
  initialGeneration?: string
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

  const thumbs = writable<ThumbMap>(new Map())
  const requested = new Set<string>()
  const highQueue: string[] = []
  const lowQueue: string[] = []
  let active = 0
  const retries = new Map<string, number>()

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const path = observed.get(entry.target)
          if (path) {
            enqueue(path, 'high')
            observer.unobserve(entry.target)
            observed.delete(entry.target)
          }
        }
      }
    },
    { root: null, rootMargin: '200px 0px', threshold: 0.01 }
  )

  const observed = new Map<Element, string>()

  function enqueue(path: string, priority: Priority = 'low') {
    if (requested.has(path)) return
    requested.add(path)
    if (priority === 'high') {
      highQueue.push(path)
    } else {
      lowQueue.push(path)
    }
    pump()
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
      }>('get_thumbnail', { path, max_dim: maxDim })
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
          setTimeout(() => enqueue(path, 'high'), delay)
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
    subscribe: thumbs.subscribe as Readable<ThumbMap>['subscribe'],
  }
}

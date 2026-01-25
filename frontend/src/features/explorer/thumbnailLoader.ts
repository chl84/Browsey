import { invoke } from '@tauri-apps/api/core'
import { writable, type Readable } from 'svelte/store'

type Options = {
  maxConcurrent?: number
  maxDim?: number
}

type ThumbMap = Map<string, string>

const DEFAULT_CONCURRENCY = 2
const DEFAULT_DIM = 96

type ObsEntry = {
  node: Element
  path: string
}

export function createThumbnailLoader(opts: Options = {}) {
  const maxConcurrent = opts.maxConcurrent ?? DEFAULT_CONCURRENCY
  const maxDim = opts.maxDim ?? DEFAULT_DIM

  const thumbs = writable<ThumbMap>(new Map())
  const requested = new Set<string>()
  const queue: string[] = []
  let active = 0

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const path = observed.get(entry.target)
          if (path) {
            enqueue(path)
            observer.unobserve(entry.target)
            observed.delete(entry.target)
          }
        }
      }
    },
    { root: null, rootMargin: '200px 0px', threshold: 0.01 }
  )

  const observed = new Map<Element, string>()

  function enqueue(path: string) {
    if (requested.has(path)) return
    requested.add(path)
    queue.push(path)
    pump()
  }

  function pump() {
    while (active < maxConcurrent && queue.length > 0) {
      const path = queue.shift()
      if (!path) break
      active++
      invoke<{
        path: string
        width: number
        height: number
        cached: boolean
      }>('get_thumbnail', { path, max_dim: maxDim })
        .then((res) => {
          thumbs.update((m) => {
            const next = new Map(m)
            next.set(path, res.path)
            return next
          })
        })
        .catch(() => {
          // swallow errors; fallback icon stays and allow future retries
          requested.delete(path)
        })
        .finally(() => {
          active--
          pump()
        })
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
    subscribe: thumbs.subscribe as Readable<ThumbMap>['subscribe'],
  }
}

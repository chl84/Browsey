import { invoke } from '@tauri-apps/api/core'
import { writable, type Readable } from 'svelte/store'
import { renderPdfFirstPage } from './pdfThumbnail'

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
      const ext = path.split('.').pop()?.toLowerCase()
      const isPdf = ext === 'pdf'
      const job = isPdf ? loadPdfThumb(path) : loadRegularThumb(path)

      job
        .then((thumbPath) => {
          if (!thumbPath) return
          thumbs.update((m) => {
            const next = new Map(m)
            next.set(path, thumbPath)
            return next
          })
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

  async function loadRegularThumb(path: string): Promise<string | null> {
    const res = await invoke<{
      path: string
      width: number
      height: number
      cached: boolean
    }>('get_thumbnail', { path, max_dim: maxDim })
    return res.path
  }

  async function loadPdfThumb(path: string): Promise<string | null> {
    const plan = await invoke<{
      cache_path: string
      cached: boolean
      width: number | null
      height: number | null
    }>('plan_pdf_thumbnail', { path, max_dim: maxDim })

    if (plan.cached) return plan.cache_path

    const data = await invoke<Uint8Array>('read_pdf_bytes', { path })
    const { blob, width, height } = await renderPdfFirstPage(data, maxDim)
    const arr = new Uint8Array(await blob.arrayBuffer())
    const pngVec = Array.from(arr)

    const res = await invoke<{
      path: string
      width: number
      height: number
      cached: boolean
    }>('store_pdf_thumbnail', {
      path,
      max_dim: maxDim,
      png: pngVec,
      width,
      height,
    })

    return res.path
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

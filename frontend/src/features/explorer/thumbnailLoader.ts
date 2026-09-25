import { invoke } from '@/shared/lib/tauri'
import { getErrorMessage } from '@/shared/lib/error'
import { writable, type Readable } from 'svelte/store'

type Options = {
  maxConcurrent?: number
  maxConcurrentVideos?: number
  maxDim?: number
  initialGeneration?: string
  allowVideos?: boolean
  allowCloudThumbs?: boolean
}
type ThumbMap = Map<string, string>
type Observation = { path: string; near: boolean; visible: boolean }
type Job = { id: string; epoch: number; video: boolean; dimension: number }
const imageExtensions = new Set([
  'png', 'jpg', 'jpeg', 'jpe', 'jfif', 'gif', 'bmp', 'ico', 'pnm', 'pbm', 'pgm', 'ppm',
  'pam', 'tga', 'webp', 'tif', 'tiff', 'hdr', 'exr', 'dds', 'svg', 'pdf',
])
const videoExtensions = new Set(['mp4', 'mov', 'm4v', 'webm', 'mkv', 'avi'])
const cloudExtensions = new Set(['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'tif', 'tiff', 'svg', 'pdf'])

export function createThumbnailLoader(opts: Options = {}) {
  const maxConcurrent = Math.max(1, opts.maxConcurrent ?? 4)
  const maxVideos = Math.max(0, Math.min(opts.maxConcurrentVideos ?? 1, maxConcurrent))
  let maxDim = opts.maxDim ?? 96
  const loaderId = crypto.randomUUID()
  let sequence = 0
  let epoch = 0
  let generation = opts.initialGeneration ?? 'init'
  let allowVideos = opts.allowVideos ?? true
  let allowCloudThumbs = opts.allowCloudThumbs ?? false
  let destroyed = false
  let scheduled = false
  const thumbs = writable<ThumbMap>(new Map())
  const loaded = new Map<string, string>()
  const loadedSizes = new Map<string, number>()
  const revisions = new Map<string, string>()
  const failedUntil = new Map<string, number>()
  const observed = new Map<Element, Observation>()
  const active = new Map<string, Job>()
  const retries = new Map<string, number>()
  const retryTimers = new Map<string, ReturnType<typeof setTimeout>>()
  const extOf = (path: string) => path.split('.').pop()?.toLowerCase() ?? ''
  const isVideo = (path: string) => videoExtensions.has(extOf(path))
  const eligible = (path: string) => path.startsWith('rclone://')
    ? allowCloudThumbs && cloudExtensions.has(extOf(path))
    : imageExtensions.has(extOf(path)) || (allowVideos && maxVideos > 0 && isVideo(path))

  const publish = () => thumbs.set(new Map(loaded))
  const wanted = (path: string) => [...observed.values()].some(o => o.path === path && o.near)
  const cancel = (path: string) => {
    const job = active.get(path)
    if (!job) return
    active.delete(path)
    // Late replies are ignored by job identity. Backend worker permits remain
    // occupied until outstanding filesystem I/O has actually stopped.
    void invoke('cancel_task', { id: job.id }).catch(() => {})
  }
  const clearRetry = (path: string) => {
    clearTimeout(retryTimers.get(path))
    retryTimers.delete(path)
  }
  const schedule = () => {
    if (scheduled || destroyed) return
    scheduled = true
    queueMicrotask(() => { scheduled = false; pump() })
  }

  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const observation = observed.get(entry.target)
      if (!observation) continue
      observation.near = entry.isIntersecting
      const rect = entry.boundingClientRect ?? entry.target.getBoundingClientRect()
      const height = window.innerHeight || document.documentElement.clientHeight
      observation.visible = entry.isIntersecting && rect.bottom > 0 && rect.top < height
    }
    schedule()
  }, { root: null, rootMargin: '200px 0px', threshold: 0.01 })
  const visibleObserver = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const observation = observed.get(entry.target)
      if (observation) observation.visible = entry.isIntersecting
    }
    schedule()
  }, { root: null, threshold: 0.01 })

  function pump() {
    if (destroyed) return
    for (const path of active.keys()) {
      if (!wanted(path) || !eligible(path)) cancel(path)
    }
    const candidates = new Map<string, number>()
    for (const { path, near, visible } of observed.values()) {
      if (!near || !eligible(path) || (loaded.has(path) && (loadedSizes.get(path) ?? 0) >= maxDim) || active.has(path)
        || retryTimers.has(path) || (failedUntil.get(path) ?? 0) > Date.now()) continue
      const priority = (visible ? 0 : 2) + (isVideo(path) ? 1 : 0)
      candidates.set(path, Math.min(candidates.get(path) ?? Infinity, priority))
    }
    for (const [path] of [...candidates].sort((a, b) => a[1] - b[1])) {
      if (active.size >= maxConcurrent) break
      if (isVideo(path) && [...active.values()].filter(job => job.video).length >= maxVideos) continue
      const job: Job = { id: `thumb-${loaderId}-${++sequence}`, epoch, video: isVideo(path), dimension: maxDim }
      active.set(path, job)
      void load(path, job)
    }
  }

  async function load(path: string, job: Job) {
    try {
      const result = await invoke<{ path: string }>('get_thumbnail', {
        path, maxDim: job.dimension, generation, requestId: job.id,
      })
      if (destroyed || job.epoch !== epoch || active.get(path) !== job) return
      loaded.set(path, result.path)
      loadedSizes.set(path, job.dimension)
      retries.delete(path)
      failedUntil.delete(path)
      publish()
    } catch (error) {
      if (destroyed || job.epoch !== epoch || active.get(path) !== job) return
      const busy = getErrorMessage(error).toLowerCase().includes('too many concurrent thumbnails')
      const attempt = (retries.get(path) ?? 0) + 1
      if (busy && attempt <= 3) {
        retries.set(path, attempt)
        retryTimers.set(path, setTimeout(() => {
          retryTimers.delete(path)
          schedule()
        }, 300 * 2 ** (attempt - 1)))
      } else {
        // Do not retry a bad file on every scroll/metadata refresh.
        failedUntil.set(path, Date.now() + 5000)
        retries.delete(path)
      }
    } finally {
      if (active.get(path) === job) active.delete(path)
      schedule()
    }
  }

  function invalidate(path: string) {
    cancel(path)
    clearRetry(path)
    retries.delete(path)
    failedUntil.delete(path)
    loaded.delete(path)
    loadedSizes.delete(path)
    publish()
    schedule()
  }

  function checkRevision(path: string, revision?: string) {
    if (revision === undefined) return
    const previous = revisions.get(path)
    revisions.set(path, revision)
    if (previous !== undefined && previous !== revision) invalidate(path)
  }

  function observe(node: Element, path: string, revision?: string) {
    if (destroyed) return { update() {}, destroy() {} }
    checkRevision(path, revision)
    observed.set(node, { path, near: false, visible: false })
    observer.observe(node)
    visibleObserver.observe(node)
    return {
      update(newPath: string, revision?: string) {
        checkRevision(newPath, revision)
        const previous = observed.get(node)
        if (previous) observed.set(node, { ...previous, path: newPath })
        schedule()
      },
      destroy() {
        observer.unobserve(node)
        visibleObserver.unobserve(node)
        observed.delete(node)
        schedule()
      },
    }
  }

  const reset = (token?: string) => {
    epoch++ // Unique even for A -> B -> A navigation.
    generation = `${loaderId}:${epoch}:${token ?? ''}`
    for (const path of active.keys()) cancel(path)
    for (const path of retryTimers.keys()) clearRetry(path)
    retries.clear()
    failedUntil.clear()
    loaded.clear()
    loadedSizes.clear()
    revisions.clear()
    publish()
    // Prevent submitting old cards before Svelte replaces the directory.
    for (const observation of observed.values()) observation.near = false
    for (const node of observed.keys()) {
      observer.unobserve(node)
      observer.observe(node)
    }
  }
  const refreshEligibility = () => {
    for (const path of loaded.keys()) if (!eligible(path)) loaded.delete(path)
    for (const [node, observation] of observed) {
      const rect = node.getBoundingClientRect()
      observation.near = rect.bottom >= -200 && rect.top <= window.innerHeight + 200
      observation.visible = rect.bottom > 0 && rect.top < window.innerHeight
    }
    publish()
    schedule()
  }
  return {
    observe,
    reset,
    setMaxDim(value: number) {
      const next = Math.max(32, Math.min(512, Math.ceil(value)))
      if (!Number.isFinite(next) || next === maxDim) return
      maxDim = next
      for (const [path, job] of active) if (job.dimension < maxDim) cancel(path)
      // Retain the old preview while a higher-resolution replacement loads.
      failedUntil.clear()
      schedule()
    },
    setAllowVideos(value: boolean) { allowVideos = value; refreshEligibility() },
    setAllowCloudThumbs(value: boolean) { allowCloudThumbs = value; refreshEligibility() },
    drop(path: string) {
      cancel(path)
      clearRetry(path)
      retries.delete(path)
      loaded.delete(path)
      failedUntil.set(path, Date.now() + 5000)
      publish()
    },
    invalidate,
    destroy() {
      destroyed = true
      reset()
      observer.disconnect()
      visibleObserver.disconnect()
      observed.clear()
    },
    subscribe: thumbs.subscribe as Readable<ThumbMap>['subscribe'],
  }
}

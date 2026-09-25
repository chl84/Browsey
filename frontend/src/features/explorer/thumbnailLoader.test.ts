import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

const invokeMock = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>(async () => ({
  path: '/tmp/thumb.png',
  width: 96,
  height: 96,
  cached: false,
}))

vi.mock('@/shared/lib/tauri', () => ({
  invoke: invokeMock,
}))

type ObserverEntry = {
  isIntersecting: boolean
  target: Element
}

class FakeIntersectionObserver {
  private callback: (entries: ObserverEntry[]) => void

  constructor(callback: (entries: ObserverEntry[]) => void) {
    this.callback = callback
    observers.push(this)
  }

  observe = (node: Element) => {
    observedNodes.add(node)
  }

  unobserve = (node: Element) => {
    observedNodes.delete(node)
  }

  disconnect = () => {
    observedNodes.clear()
  }

  trigger(node: Element, isIntersecting = true) {
    this.callback([{ isIntersecting, target: node }])
  }
}

const observers: FakeIntersectionObserver[] = []
const observedNodes = new Set<Element>()

const createNode = () => {
  const node = document.createElement('div')
  node.getBoundingClientRect = () =>
    ({
      top: 0,
      bottom: 10,
      left: 0,
      right: 10,
      width: 10,
      height: 10,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    }) as DOMRect
  return node
}

describe('createThumbnailLoader cloud eligibility', () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue({ path: '/tmp/thumb.png', width: 96, height: 96, cached: false })
    observers.length = 0
    observedNodes.clear()
    ;(globalThis as { IntersectionObserver: unknown }).IntersectionObserver =
      FakeIntersectionObserver as unknown as typeof IntersectionObserver
  })
  afterEach(() => vi.useRealTimers())

  it('never queues unsupported local files or codecs not built into Browsey', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader()
    for (const ext of ['txt', 'zip', 'docx', 'heic', 'avif']) {
      const node = createNode()
      loader.observe(node, `/mock/file.${ext}`)
      observers[0].trigger(node)
    }
    await Promise.resolve()
    expect(invokeMock).not.toHaveBeenCalled()
    loader.destroy()
  })

  it('reuses loaded thumbnails when a card is remounted and invalidates changed files', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader()
    const node = createNode()
    const binding = loader.observe(node, '/mock/photo.jpg')
    observers[0].trigger(node)
    await vi.waitFor(() => expect(get(loader).size).toBe(1))
    binding.destroy()
    loader.observe(node, '/mock/photo.jpg')
    observers[0].trigger(node)
    await Promise.resolve()
    expect(invokeMock).toHaveBeenCalledTimes(1)
    loader.invalidate('/mock/photo.jpg')
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2))
    loader.destroy()
  })

  it('starts visible cards before overscan cards regardless of observer order', async () => {
    invokeMock.mockImplementation(() => new Promise(() => {}))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ maxConcurrent: 1 })
    const overscan = createNode(), visible = createNode()
    overscan.getBoundingClientRect = () => ({ top: -150, bottom: -50 }) as DOMRect
    loader.observe(overscan, '/mock/prefetch.jpg')
    loader.observe(visible, '/mock/visible.jpg')
    observers[0].trigger(overscan)
    observers[0].trigger(visible)
    await Promise.resolve()
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('get_thumbnail', expect.objectContaining({ path: '/mock/visible.jpg', maxDim: 96 }))
    loader.destroy()
  })

  it('keeps previews visible while upgrading resolution and reuses them on zoom out', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ maxDim: 64 })
    const node = createNode()
    loader.observe(node, '/mock/photo.jpg')
    observers[0].trigger(node)
    await vi.waitFor(() => expect(get(loader).size).toBe(1))
    let finish!: (value: unknown) => void
    invokeMock.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    loader.setMaxDim(192)
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2))
    expect(get(loader).get('/mock/photo.jpg')).toBe('/tmp/thumb.png')
    expect(invokeMock).toHaveBeenLastCalledWith('get_thumbnail', expect.objectContaining({ maxDim: 192 }))
    finish({ path: '/tmp/large-thumb.png' })
    await vi.waitFor(() => expect(get(loader).get('/mock/photo.jpg')).toBe('/tmp/large-thumb.png'))
    loader.setMaxDim(64)
    await Promise.resolve()
    expect(invokeMock).toHaveBeenCalledTimes(2)
    loader.destroy()
  })

  it('invalidates cached cards if a file changes while scrolled out of view', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader()
    const node = createNode()
    const binding = loader.observe(node, '/mock/photo.jpg', 'revision-1')
    observers[0].trigger(node)
    await vi.waitFor(() => expect(get(loader).size).toBe(1))
    binding.destroy()
    loader.observe(node, '/mock/photo.jpg', 'revision-2')
    observers[0].trigger(node)
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2))
    loader.destroy()
  })

  it('drops queued cards that leave the viewport', async () => {
    let finish!: (value: unknown) => void
    invokeMock.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ maxConcurrent: 1 })
    const first = createNode(), stale = createNode(), visible = createNode()
    loader.observe(first, '/mock/first.jpg')
    observers[0].trigger(first)
    await Promise.resolve()
    const old = loader.observe(stale, '/mock/scrolled-away.jpg')
    observers[0].trigger(stale)
    loader.observe(visible, '/mock/visible.jpg')
    observers[0].trigger(visible)
    old.destroy()
    finish({ path: '/cache/first.png' })
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2))
    expect(invokeMock).toHaveBeenLastCalledWith('get_thumbnail', expect.objectContaining({ path: '/mock/visible.jpg' }))
    loader.destroy()
  })

  it('cancels offscreen active work and uses the slot for visible work', async () => {
    invokeMock.mockImplementation(() => new Promise(() => {}))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ maxConcurrent: 1 })
    const old = createNode(), current = createNode()
    loader.observe(old, '/mock/old.jpg')
    observers[0].trigger(old)
    await Promise.resolve()
    observers[0].trigger(old, false)
    loader.observe(current, '/mock/current.jpg')
    observers[0].trigger(current)
    await Promise.resolve()
    expect(invokeMock.mock.calls.map(call => call[0])).toEqual(['get_thumbnail', 'cancel_task', 'get_thumbnail'])
    loader.destroy()
  })

  it('navigation releases old slots and ignores stale A -> B -> A replies', async () => {
    const pending: Array<(value: unknown) => void> = []
    invokeMock.mockImplementation((cmd) => cmd === 'cancel_task' ? Promise.resolve() : new Promise(resolve => pending.push(resolve)))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ maxConcurrent: 1, initialGeneration: 'A' })
    const node = createNode()
    loader.observe(node, '/A/photo.jpg')
    observers[0].trigger(node)
    await Promise.resolve()
    loader.reset('B')
    loader.reset('A')
    observers[0].trigger(node)
    await Promise.resolve()
    expect(pending).toHaveLength(2)
    pending[0]({ path: '/stale.png' })
    await Promise.resolve()
    expect(get(loader).size).toBe(0)
    pending[1]({ path: '/current.png' })
    await vi.waitFor(() => expect(get(loader).get('/A/photo.jpg')).toBe('/current.png'))
    loader.destroy()
  })

  it('does not repeatedly request failed files on card remount', async () => {
    invokeMock.mockRejectedValue(new Error('Unsupported image format'))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader()
    const node = createNode()
    const old = loader.observe(node, '/mock/bad.jpg')
    observers[0].trigger(node)
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    old.destroy()
    loader.observe(node, '/mock/bad.jpg')
    observers[0].trigger(node)
    await Promise.resolve()
    expect(invokeMock).toHaveBeenCalledTimes(1)
    loader.destroy()
  })

  it('respects the separate video limit while allowing image requests', async () => {
    invokeMock.mockImplementation(() => new Promise(() => {}))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ maxConcurrent: 3, maxConcurrentVideos: 1 })
    for (const path of ['/a.mp4', '/b.mp4', '/c.jpg', '/d.jpg']) {
      const node = createNode(); loader.observe(node, path); observers[0].trigger(node)
    }
    await Promise.resolve()
    expect(invokeMock.mock.calls.map(call => call[1]?.path)).toEqual(['/c.jpg', '/d.jpg', '/a.mp4'])
    loader.destroy()
  })

  it('bounds overload retries and cancels retry timers when destroyed', async () => {
    vi.useFakeTimers()
    invokeMock.mockRejectedValue(new Error('Too many concurrent thumbnails'))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader()
    const node = createNode(); loader.observe(node, '/mock/photo.jpg'); observers[0].trigger(node)
    await vi.advanceTimersByTimeAsync(2200)
    expect(invokeMock).toHaveBeenCalledTimes(4)
    loader.destroy()
    await vi.advanceTimersByTimeAsync(10000)
    expect(invokeMock).toHaveBeenCalledTimes(4)
  })

  it('does not enqueue cloud thumbnails when allowCloudThumbs is false', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ allowCloudThumbs: false, allowVideos: true, maxConcurrent: 1 })
    const node = createNode()

    loader.observe(node, 'rclone://work/docs/photo.png')
    observers[0]?.trigger(node, true)
    await Promise.resolve()

    expect(invokeMock).not.toHaveBeenCalled()
    loader.destroy()
  })

  it('enqueues allowed cloud image thumbnails when allowCloudThumbs is true', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ allowCloudThumbs: true, allowVideos: true, maxConcurrent: 1 })
    const node = createNode()

    loader.observe(node, 'rclone://work/docs/photo.png')
    observers[0]?.trigger(node, true)

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('get_thumbnail', expect.objectContaining({ path: 'rclone://work/docs/photo.png' }))
    })
    loader.destroy()
  })

  it('blocks cloud video thumbnails even when cloud thumbs are enabled', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ allowCloudThumbs: true, allowVideos: true, maxConcurrent: 1 })
    const node = createNode()

    loader.observe(node, 'rclone://work/docs/video.mp4')
    observers[0]?.trigger(node, true)
    await Promise.resolve()

    expect(invokeMock).not.toHaveBeenCalled()
    loader.destroy()
  })

  it('still enqueues local thumbnails when cloud thumbs are disabled', async () => {
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ allowCloudThumbs: false, allowVideos: true, maxConcurrent: 1 })
    const node = createNode()

    loader.observe(node, '/home/chris/photo.png')
    observers[0]?.trigger(node, true)

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('get_thumbnail', expect.objectContaining({ path: '/home/chris/photo.png' }))
    })
    loader.destroy()
  })

  it('falls back cleanly when a local video thumbnail request fails', async () => {
    invokeMock.mockRejectedValueOnce(new Error('ffmpeg not found in PATH'))
    const { createThumbnailLoader } = await import('./thumbnailLoader')
    const loader = createThumbnailLoader({ allowCloudThumbs: false, allowVideos: true, maxConcurrent: 1 })
    const node = createNode()

    loader.observe(node, '/home/chris/video.mp4')
    observers[0]?.trigger(node, true)

    await Promise.resolve()
    await Promise.resolve()

    expect(invokeMock).toHaveBeenCalledWith(
      'get_thumbnail',
      expect.objectContaining({ path: '/home/chris/video.mp4' }),
    )
    let seenThumb: string | null = null
    const unsubscribe = loader.subscribe((thumbs) => {
      seenThumb = thumbs.get('/home/chris/video.mp4') ?? null
    })
    expect(seenThumb).toBeNull()
    unsubscribe()
    loader.destroy()
  })
})

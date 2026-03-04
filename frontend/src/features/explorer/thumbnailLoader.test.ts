import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn(async () => ({
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
    vi.clearAllMocks()
    observers.length = 0
    observedNodes.clear()
    ;(globalThis as { IntersectionObserver: unknown }).IntersectionObserver =
      FakeIntersectionObserver as unknown as typeof IntersectionObserver
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
})

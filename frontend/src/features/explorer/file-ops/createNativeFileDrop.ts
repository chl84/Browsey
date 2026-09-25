import { writable } from 'svelte/store'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'

export type DropPosition = { x: number; y: number }
type Options = {
  onHover?: (paths: string[], position: DropPosition) => void
  onLeave?: () => void
  onDrop?: (paths: string[], position: DropPosition) => void | Promise<void>
  onError?: (error: unknown) => void
}

// Tauri reports physical pixels; DOM hit testing and the drag ghost use CSS pixels.
export const logicalDropPosition = (position: DropPosition): DropPosition => ({
  x: position.x / (window.devicePixelRatio || 1),
  y: position.y / (window.devicePixelRatio || 1),
})

export const createNativeFileDrop = (options: Options = {}) => {
  const hovering = writable(false)
  const position = writable<DropPosition | null>(null)
  let unlisten: UnlistenFn | null = null
  let starting: Promise<void> | null = null
  let generation = 0
  let paths: string[] = []
  const reset = () => {
    paths = []
    hovering.set(false)
    position.set(null)
  }

  const start = (): Promise<void> => {
    if (unlisten) return Promise.resolve()
    if (starting) return starting
    const token = ++generation
    const request = (async () => {
      const webview = await getCurrentWebview()
      const listener = await webview.onDragDropEvent((event) => {
        if (token !== generation) return
        const payload = event.payload
        if (payload.type === 'enter' || payload.type === 'over') {
          if (payload.type === 'enter') paths = [...payload.paths]
          const point = logicalDropPosition(payload.position)
          hovering.set(true)
          position.set(point)
          options.onHover?.(paths, point)
        } else if (payload.type === 'drop') {
          reset()
          try {
            void Promise.resolve(options.onDrop?.([...payload.paths], logicalDropPosition(payload.position)))
              .catch(options.onError ?? (() => {}))
          } catch (error) { options.onError?.(error) }
        } else {
          reset()
          options.onLeave?.()
        }
      })
      if (token !== generation) listener()
      else unlisten = listener
    })()
    starting = request
    void request.finally(() => {
      if (starting === request) starting = null
    }).catch(() => {})
    return request
  }

  const stop = async () => {
    generation += 1
    starting = null
    unlisten?.()
    unlisten = null
    reset()
    options.onLeave?.()
  }
  return { hovering, position, start, stop }
}

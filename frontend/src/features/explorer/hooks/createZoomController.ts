import { nextZoomState, type ZoomState } from './createWheelZoom'

type Options = {
  getState: () => ZoomState
  apply: (state: ZoomState) => Promise<void>
  isBlocked: () => boolean
  onError: (error: unknown) => void
}

const sameState = (a: ZoomState, b: ZoomState) => a.viewMode === b.viewMode && a.size === b.size

// Keep the desired state separate from the rendered state: wheel events may
// arrive during a frame or an asynchronous list/grid switch.
export const createZoomController = ({ getState, apply, isBlocked, onError }: Options) => {
  let desired: ZoomState | null = null
  let frame: number | null = null
  let applying = false
  let destroyed = false

  const clearPending = () => {
    desired = null
    if (frame !== null) cancelAnimationFrame(frame)
    frame = null
  }

  const schedule = () => {
    if (destroyed || applying || frame !== null || !desired) return
    frame = requestAnimationFrame(() => {
      frame = null
      if (destroyed || isBlocked()) {
        clearPending()
        return
      }
      const next = desired
      if (!next || sameState(next, getState())) {
        desired = null
        return
      }
      applying = true
      void (async () => {
        try {
          await apply(next)
        } catch (error) {
          clearPending()
          if (!destroyed) onError(error)
        } finally {
          applying = false
          if (desired === next) desired = null
          schedule()
        }
      })()
    })
  }

  return {
    step(direction: -1 | 1) {
      if (destroyed) return
      if (isBlocked()) {
        clearPending()
        return
      }
      const current = desired ?? getState()
      const next = nextZoomState(current.viewMode, current.size, direction)
      if (sameState(current, next)) return
      desired = next
      schedule()
    },
    clearPending,
    destroy() {
      destroyed = true
      clearPending()
    },
  }
}

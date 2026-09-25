export const GRID_ZOOM_SIZES = [64, 96, 128, 160, 192] as const
export const DEFAULT_GRID_ZOOM_SIZE = 96
type ViewMode = 'list' | 'grid'

export const nextZoomState = (viewMode: ViewMode, size: number, direction: -1 | 1) => {
  if (viewMode === 'list') return direction > 0 ? { viewMode: 'grid' as const, size: GRID_ZOOM_SIZES[0] } : { viewMode, size }
  const index = Math.max(0, GRID_ZOOM_SIZES.findIndex(value => value >= size))
  const next = index + direction
  if (next < 0) return { viewMode: 'list' as const, size: GRID_ZOOM_SIZES[0] }
  return { viewMode, size: GRID_ZOOM_SIZES[Math.min(next, GRID_ZOOM_SIZES.length - 1)] }
}

export const createWheelZoom = (onStep: (direction: -1 | 1) => void, isBlocked: () => boolean = () => false) => {
  let accumulated = 0
  let lastEvent = -Infinity
  let lastStep = -Infinity
  let previousDirection = 0
  return (event: WheelEvent) => {
    if (!event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || event.defaultPrevented) return false
    if (event.cancelable) event.preventDefault()
    event.stopPropagation()
    if (isBlocked() || !Number.isFinite(event.deltaY) || event.deltaY === 0) return true
    const now = performance.now()
    const direction = event.deltaY < 0 ? 1 : -1
    if (now - lastEvent > 180 || direction !== previousDirection) accumulated = 0
    const reversed = direction !== previousDirection
    lastEvent = now
    previousDirection = direction
    // Normalize trackpad pixels, mouse lines and page-based wheel events.
    const multiplier = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 120 : 1
    accumulated += Math.abs(event.deltaY) * multiplier
    if (accumulated >= 48 && (reversed || now - lastStep >= 80)) {
      accumulated = 0
      lastStep = now
      onStep(direction)
    }
    return true
  }
}

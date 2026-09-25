import type { DropPosition } from './createNativeFileDrop'
import type { DropTarget } from './dropTargets'

export const createDragNavigation = (options: {
  canNavigate: () => boolean
  open: (path: string) => void
  onScroll: (point: DropPosition) => void
}) => {
  let hoverTimer: ReturnType<typeof setTimeout> | null = null
  let hoverPath: string | null = null
  let frame: number | null = null
  let point: DropPosition | null = null
  let scrollElement: HTMLElement | null = null
  let previousTime = 0

  const clearHover = () => {
    if (hoverTimer !== null) clearTimeout(hoverTimer)
    hoverTimer = null
    hoverPath = null
  }
  const stop = () => {
    clearHover()
    if (frame !== null) cancelAnimationFrame(frame)
    frame = null
    point = null
    scrollElement = null
    previousTime = 0
  }
  const scroll = (time: number) => {
    frame = null
    if (!point || !scrollElement || !scrollElement.isConnected || !options.canNavigate()) {
      stop()
      return
    }
    const rect = scrollElement.getBoundingClientRect()
    const edge = Math.min(48, rect.height / 4)
    const inside = point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
    const speed = !inside || !edge ? 0 : point.y < rect.top + edge
      ? -(1 - (point.y - rect.top) / edge)
      : point.y > rect.bottom - edge ? 1 - (rect.bottom - point.y) / edge : 0
    const delta = Math.min(32, previousTime ? time - previousTime : 16)
    previousTime = time
    const before = scrollElement.scrollTop
    scrollElement.scrollTop += speed * delta * 0.6
    if (before !== scrollElement.scrollTop) {
      clearHover()
      options.onScroll(point)
    }
    if (point && frame === null) frame = requestAnimationFrame(scroll)
  }
  const update = (next: DropPosition, target: DropTarget | null) => {
    point = next
    scrollElement = document.elementFromPoint(next.x, next.y)?.closest<HTMLElement>('[data-drop-scroll]') ?? null
    if (!scrollElement && frame !== null) {
      cancelAnimationFrame(frame)
      frame = null
      previousTime = 0
    }
    const path = target?.openOnHover ? target.path : null
    if (path !== hoverPath) {
      clearHover()
      hoverPath = path
      if (path) hoverTimer = setTimeout(() => {
        hoverTimer = null
        if (options.canNavigate()) options.open(path)
      }, 850)
    }
    if (scrollElement && frame === null) frame = requestAnimationFrame(scroll)
  }
  return { update, stop }
}

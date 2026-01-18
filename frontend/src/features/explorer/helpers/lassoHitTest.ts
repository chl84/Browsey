export type Rect = { x: number; y: number; width: number; height: number }

export const hitTestList = (
  rect: Rect,
  rowsEl: HTMLDivElement,
  headerHeight: number,
  rowHeight: number,
  entries: { path: string }[],
) => {
  const yStartContent = rect.y - headerHeight
  const yEndContent = rect.y + rect.height - headerHeight
  if (yEndContent < 0) return { paths: new Set<string>(), anchor: null, caret: null }
  const lo = Math.max(0, Math.floor(yStartContent / rowHeight))
  const hi = Math.max(0, Math.floor(yEndContent / rowHeight))
  const paths = new Set<string>()
  for (let i = lo; i <= hi && i < entries.length; i++) {
    const path = entries[i]?.path
    if (path) paths.add(path)
  }
  return { paths, anchor: lo, caret: hi }
}

export const hitTestGrid = (rect: Rect, container: HTMLDivElement) => {
  const containerRect = container.getBoundingClientRect()
  const cards = Array.from(container.querySelectorAll<HTMLButtonElement>('.card'))
  const paths = new Set<string>()
  for (const card of cards) {
    const box = card.getBoundingClientRect()
    const cx1 = box.left - containerRect.left + container.scrollLeft
    const cy1 = box.top - containerRect.top + container.scrollTop
    const cx2 = cx1 + box.width
    const cy2 = cy1 + box.height
    const rx2 = rect.x + rect.width
    const ry2 = rect.y + rect.height
    const intersects = cx1 < rx2 && cx2 > rect.x && cy1 < ry2 && cy2 > rect.y
    if (intersects) {
      const path = card.dataset.path
      if (path) paths.add(path)
    }
  }
  return { paths, anchor: null, caret: null }
}

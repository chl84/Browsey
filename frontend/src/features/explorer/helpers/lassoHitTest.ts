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
  const hi = Math.max(0, Math.floor((yEndContent - 0.001) / rowHeight))
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

type GridMetrics = {
  gridCols: number
  cardWidth: number
  cardHeight: number
  gap: number
  padding: number
}

export const hitTestGridVirtualized = (rect: Rect, entries: { path: string }[], metrics: GridMetrics) => {
  const { gridCols, cardWidth, cardHeight, gap, padding } = metrics
  if (gridCols <= 0 || cardWidth <= 0 || cardHeight <= 0) {
    return { paths: new Set<string>(), anchor: null, caret: null }
  }

  const rectRight = rect.x + rect.width
  const rectBottom = rect.y + rect.height
  const x0 = Math.max(0, rect.x - padding)
  // Juster y for scroll/oversettelse: rektangelet gis i viewport-koordinater,
  // rows start at padding and are not transformed by translateY in the hit test.
  const y0 = Math.max(0, rect.y - padding)
  const colStride = cardWidth + gap
  const rowStride = cardHeight + gap
  // Bruk gulv for start og tak for slutt slik at vi inkluderer nedre rad selv ved avrundingsfeil.
  const startRow = Math.max(0, Math.floor(y0 / rowStride))
  const endRow = Math.max(startRow, Math.ceil((y0 + rect.height) / rowStride) - 1)
  const startCol = Math.max(0, Math.floor(x0 / colStride))
  const endCol = Math.max(startCol, Math.ceil((x0 + rect.width) / colStride) - 1)

  const paths = new Set<string>()
  let anchor: number | null = null
  let caret: number | null = null

  for (let row = startRow; row <= endRow; row++) {
    for (let col = startCol; col <= endCol; col++) {
      if (col >= gridCols) continue
      const idx = row * gridCols + col
      const entry = entries[idx]
      if (!entry || !entry.path) continue

      const cardLeft = padding + col * colStride
      const cardRight = cardLeft + cardWidth
      const cardTop = padding + row * rowStride
      const cardBottom = cardTop + cardHeight
      const intersects =
        cardLeft < rectRight &&
        cardRight > rect.x &&
        cardTop < rectBottom &&
        cardBottom > rect.y
      if (!intersects) continue

      paths.add(entry.path)
      if (anchor === null) anchor = idx
      caret = idx
    }
  }

  return { paths, anchor, caret }
}

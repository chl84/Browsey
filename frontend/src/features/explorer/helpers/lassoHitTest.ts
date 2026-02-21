type Rect = { x: number; y: number; width: number; height: number }

type GridMetrics = {
  gridCols: number
  cardWidth: number
  cardHeight: number
  gap: number
  paddingLeft: number
  paddingTop: number
}

export const hitTestGridVirtualized = (rect: Rect, entries: { path: string }[], metrics: GridMetrics) => {
  const { gridCols, cardWidth, cardHeight, gap, paddingLeft, paddingTop } = metrics
  if (gridCols <= 0 || cardWidth <= 0 || cardHeight <= 0) {
    return { paths: new Set<string>(), anchor: null, caret: null }
  }

  const rectRight = rect.x + rect.width
  const rectBottom = rect.y + rect.height
  const x0 = Math.max(0, rect.x - paddingLeft)
  // Juster y for scroll/oversettelse: rektangelet gis i viewport-koordinater,
  // rows start at padding and are not transformed by translateY in the hit test.
  const y0 = Math.max(0, rect.y - paddingTop)
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

      const cardLeft = paddingLeft + col * colStride
      const cardRight = cardLeft + cardWidth
      const cardTop = paddingTop + row * rowStride
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

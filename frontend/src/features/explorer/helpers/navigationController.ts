export type NavigateArgs = {
  count: number
  current: number | null
  delta?: number
  toStart?: boolean
  toEnd?: boolean
}

export const moveCaret = ({ count, current, delta = 0, toStart, toEnd }: NavigateArgs) => {
  if (count === 0) return null
  if (toStart) return 0
  if (toEnd) return count - 1
  const base = current ?? 0
  const next = Math.min(count - 1, Math.max(0, base + delta))
  return next
}

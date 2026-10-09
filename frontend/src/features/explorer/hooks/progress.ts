export type ProgressPayload = { total: number; finished?: boolean; phase?: string } & (
  | { unit?: 'bytes'; bytes: number }
  | { unit: 'items'; items: number }
)

const count = (value: number) => Number.isFinite(value) ? Math.max(0, value) : 0

export const formatProgressBytes = (value: number) => {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = count(value)
  let index = 0
  while (size >= 1024 && index < units.length - 1) { size /= 1024; index += 1 }
  const precision = index === 0 ? 0 : size >= 100 ? 0 : size >= 10 ? 1 : 2
  return `${size.toFixed(precision)} ${units[index]}`
}

/** Unknown totals still show measured work, while the bar stays indeterminate. */
export const progressPresentation = (payload: ProgressPayload) => {
  const completed = count(payload.unit === 'items' ? payload.items : payload.bytes)
  const total = count(payload.total)
  const percent = total > 0
    ? Math.min(100, completed > 0 ? Math.max(1, Math.round(completed / total * 100)) : 0)
    : null
  const detail = total > 0
    ? payload.unit === 'items'
      ? `${completed} / ${total} ${total === 1 ? 'item' : 'items'}`
      : `${formatProgressBytes(completed)} / ${formatProgressBytes(total)}`
    : completed > 0
      ? payload.unit === 'items'
        ? `${completed} ${completed === 1 ? 'item' : 'items'} completed`
        : `${formatProgressBytes(completed)} processed`
      : null
  return { percent, detail }
}

// Shared file-content size formatting; retain existing explorer units/rounding.
export const formatSize = (size?: number | null) => {
  if (size === null || size === undefined) return ''
  if (size < 1024) return `${size} B`
  const units = ['kB', 'MB', 'GB', 'TB']
  let value = size / 1000
  let u = 0
  while (value >= 1000 && u < units.length - 1) {
    value /= 1000
    u++
  }
  return `${value.toFixed(1)} ${units[u]}`
}

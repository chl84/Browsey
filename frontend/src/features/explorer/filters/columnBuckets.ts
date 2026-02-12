import type { Entry } from '../types'

export type RankedBucket = {
  label: string
  rank: number
}

export const typeLabel = (entry: Entry): string => {
  if (entry.ext && entry.ext.length > 0) return entry.ext.toLowerCase()
  if (entry.kind) return entry.kind.toLowerCase()
  return ''
}

export const modifiedBucket = (modified?: string | null): RankedBucket | null => {
  if (!modified) return null
  const iso = modified.replace(' ', 'T')
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return null
  const now = new Date()
  const msPerDay = 1000 * 60 * 60 * 24
  const diffDays = Math.floor((now.getTime() - date.getTime()) / msPerDay)
  if (diffDays <= 0) return { label: 'Today', rank: 0 }
  if (diffDays === 1) return { label: 'Yesterday', rank: 1 }
  if (diffDays < 7) return { label: `${diffDays} days ago`, rank: diffDays }
  if (diffDays < 30) {
    const weeks = Math.floor((diffDays + 6) / 7)
    return { label: weeks === 1 ? '1 week ago' : `${weeks} weeks ago`, rank: weeks * 7 }
  }
  if (diffDays < 365) {
    const months = Math.floor((diffDays + 29) / 30)
    return { label: months === 1 ? '1 month ago' : `${months} months ago`, rank: months * 30 }
  }
  const years = Math.floor((diffDays + 364) / 365)
  return { label: years === 1 ? '1 year ago' : `${years} years ago`, rank: years * 365 }
}

export const sizeBucket = (size: number): RankedBucket | null => {
  const KB = 1024
  const MB = 1024 * KB
  const GB = 1024 * MB
  const TB = 1024 * GB
  const buckets: Array<[number, string]> = [
    [10 * KB, '0–10 KB'],
    [100 * KB, '10–100 KB'],
    [MB, '100 KB–1 MB'],
    [10 * MB, '1–10 MB'],
    [100 * MB, '10–100 MB'],
    [GB, '100 MB–1 GB'],
    [10 * GB, '1–10 GB'],
    [100 * GB, '10–100 GB'],
    [TB, '100 GB–1 TB'],
  ]
  for (const [limit, label] of buckets) {
    if (size <= limit) return { label, rank: limit }
  }
  const over = Math.max(1, Math.floor(size / TB))
  return { label: 'Over 1 TB', rank: over * TB }
}

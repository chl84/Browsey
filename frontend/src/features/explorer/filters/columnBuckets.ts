import type { Entry } from '../model/types'

export type RankedBucket = {
  label: string
  rank: number
}

const MS_PER_DAY = 1000 * 60 * 60 * 24
const KB = 1024
const MB = 1024 * KB
const GB = 1024 * MB
const TB = 1024 * GB

const SIZE_BUCKETS: Array<[number, string]> = [
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

const sizeLabelRank = new Map<string, number>(SIZE_BUCKETS.map(([limit, label]) => [label, limit]))

const parseLocalDateTime = (value: string): Date | null => {
  const match = value.trim().match(/^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})$/)
  if (!match) return null

  const year = Number(match[1])
  const month = Number(match[2]) - 1
  const day = Number(match[3])
  const hour = Number(match[4])
  const minute = Number(match[5])

  const date = new Date(year, month, day, hour, minute, 0, 0)
  if (Number.isNaN(date.getTime())) return null

  // Reject overflows that Date normalizes (e.g. month 13 -> next year).
  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month ||
    date.getDate() !== day ||
    date.getHours() !== hour ||
    date.getMinutes() !== minute
  ) {
    return null
  }
  return date
}

const naiveEpochMs = (date: Date): number =>
  Date.UTC(
    date.getFullYear(),
    date.getMonth(),
    date.getDate(),
    date.getHours(),
    date.getMinutes(),
    date.getSeconds(),
    date.getMilliseconds(),
  )

const modifiedRankFromLabel = (label: string): number => {
  if (label === 'Today') return 0
  if (label === 'Yesterday') return 1

  const days = label.match(/^(\d+) days ago$/)
  if (days) return Number(days[1])

  const weeks = label.match(/^(\d+) week(?:s)? ago$/)
  if (weeks) return Number(weeks[1]) * 7

  const months = label.match(/^(\d+) month(?:s)? ago$/)
  if (months) return Number(months[1]) * 30

  const years = label.match(/^(\d+) year(?:s)? ago$/)
  if (years) return Number(years[1]) * 365

  return Number.MAX_SAFE_INTEGER
}

export const typeLabel = (entry: Entry): string => {
  if (entry.ext && entry.ext.length > 0) return entry.ext.toLowerCase()
  if (entry.kind) return entry.kind.toLowerCase()
  return ''
}

export const modifiedBucket = (modified?: string | null): RankedBucket | null => {
  if (!modified) return null
  const date = parseLocalDateTime(modified)
  if (!date) return null
  const now = new Date()
  const diffDays = Math.floor((naiveEpochMs(now) - naiveEpochMs(date)) / MS_PER_DAY)
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

export const modifiedFilterRank = (id: string): number => {
  const label = id.startsWith('modified:') ? id.slice('modified:'.length) : id
  return modifiedRankFromLabel(label)
}

export const sizeBucket = (size: number): RankedBucket | null => {
  for (const [limit, label] of SIZE_BUCKETS) {
    if (size <= limit) return { label, rank: limit }
  }
  const over = Math.max(1, Math.floor(size / TB))
  return { label: 'Over 1 TB', rank: over * TB }
}

export const sizeFilterRank = (id: string): number => {
  const label = id.startsWith('size:') ? id.slice('size:'.length) : id
  if (label === 'Over 1 TB') return Number.MAX_SAFE_INTEGER - 1
  return sizeLabelRank.get(label) ?? Number.MAX_SAFE_INTEGER
}

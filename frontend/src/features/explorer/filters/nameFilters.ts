const ORDERED_BUCKETS = [
  { id: 'name:a-f', label: 'A–F' },
  { id: 'name:g-l', label: 'G–L' },
  { id: 'name:m-r', label: 'M–R' },
  { id: 'name:s-z', label: 'S–Z' },
  { id: 'name:0-9', label: '0–9' },
  { id: 'name:other', label: 'Other symbols' },
] as const

const idToLabel: Map<string, string> = new Map(
  ORDERED_BUCKETS.map((bucket) => [bucket.id, bucket.label] as [string, string]),
)
const idToRank: Map<string, number> = new Map(
  ORDERED_BUCKETS.map((bucket, index) => [bucket.id, index] as [string, number]),
)

export const nameFilterOptions = ORDERED_BUCKETS.map((bucket) => ({ ...bucket }))

export const nameBucket = (value: string): string => {
  const ch = value.charAt(0)
  if (ch >= 'a' && ch <= 'f') return 'name:a-f'
  if (ch >= 'g' && ch <= 'l') return 'name:g-l'
  if (ch >= 'm' && ch <= 'r') return 'name:m-r'
  if (ch >= 's' && ch <= 'z') return 'name:s-z'
  if (ch >= '0' && ch <= '9') return 'name:0-9'
  return 'name:other'
}

export const nameFilterLabel = (id: string): string => idToLabel.get(id) ?? id

export const nameFilterRank = (id: string): number => idToRank.get(id) ?? Number.MAX_SAFE_INTEGER

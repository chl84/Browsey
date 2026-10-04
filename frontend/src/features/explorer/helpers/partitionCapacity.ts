const capacityNumber = new Intl.NumberFormat('en', { maximumFractionDigits: 1 })

// Disk capacities use decimal GB, matching manufacturers and lsblk --bytes.
export const partitionCapacity = (bytes?: number | null): string => {
  if (bytes == null || !Number.isFinite(bytes) || bytes <= 0) return ''
  const gb = bytes / 1_000_000_000
  return gb < 0.1 ? '<0.1 GB' : `${capacityNumber.format(gb)} GB`
}

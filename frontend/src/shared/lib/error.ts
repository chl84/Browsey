export type NormalizedError = Error & {
  code?: string
  details?: unknown
  raw?: unknown
}

type ErrorLike = {
  code?: string
  message?: string
  details?: unknown
}

const asRecord = (value: unknown): Record<string, unknown> | null => {
  if (value && typeof value === 'object') return value as Record<string, unknown>
  return null
}

const nonemptyString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim().length > 0 ? value : undefined

const errorRecord = (value: unknown): Record<string, unknown> | null => {
  if (typeof value === 'string' && value.trimStart().startsWith('{')) {
    try {
      return asRecord(JSON.parse(value))
    } catch {
      return null
    }
  }
  return asRecord(value)
}

const asErrorLike = (value: unknown): ErrorLike | null => {
  const record = errorRecord(value)
  if (!record) return null
  const nested = asRecord(record.error)
  return {
    code: (nonemptyString(record.code) ?? nonemptyString(nested?.code))?.trim(),
    message: [record.message, record.error, record.cause, nested?.message, nested?.error, nested?.cause]
      .map(nonemptyString)
      .find(message => message !== undefined),
    details: 'details' in record ? record.details : nested?.details,
  }
}

const errorMessage = (value: unknown): string => {
  // Keep plain/JSON string diagnostics unchanged; parse JSON only for metadata.
  if (typeof value === 'string') return nonemptyString(value) ?? 'Unknown error'
  const message = asErrorLike(value)?.message
  if (message) return message
  try {
    const serialized = JSON.stringify(value)
    return serialized && serialized !== '{}' ? serialized : 'Unknown error'
  } catch {
    return 'Unknown error'
  }
}

export const normalizeError = (value: unknown): NormalizedError => {
  if (value instanceof Error) {
    return value as NormalizedError
  }

  const like = asErrorLike(value)
  const error = new Error(errorMessage(value)) as NormalizedError
  if (like?.code) error.code = like.code
  if (like && 'details' in like) error.details = like.details
  error.raw = value
  return error
}

export const getErrorMessage = (value: unknown): string =>
  nonemptyString(normalizeError(value).message) ?? errorMessage(value)

// Read typed codes only, never classify prose. The bounded raw fallback also
// supports an Error already produced by the IPC wrapper without mutating it.
export const getErrorCode = (value: unknown): string | null =>
  asErrorLike(value)?.code ?? asErrorLike(asRecord(value)?.raw)?.code ?? null

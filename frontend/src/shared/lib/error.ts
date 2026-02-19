export type NormalizedError = Error & {
  code?: string
  details?: unknown
  raw?: unknown
}

type ErrorLike = {
  code?: unknown
  message?: unknown
  details?: unknown
}

const asRecord = (value: unknown): Record<string, unknown> | null => {
  if (value && typeof value === 'object') return value as Record<string, unknown>
  return null
}

const asErrorLike = (value: unknown): ErrorLike | null => {
  const record = asRecord(value)
  if (!record) return null
  return {
    code: record.code,
    message: record.message,
    details: record.details,
  }
}

export const normalizeError = (value: unknown): NormalizedError => {
  if (value instanceof Error) {
    return value as NormalizedError
  }

  const like = asErrorLike(value)
  const message =
    typeof like?.message === 'string'
      ? like.message
      : typeof value === 'string'
        ? value
        : (() => {
            try {
              return JSON.stringify(value)
            } catch {
              return String(value)
            }
          })()

  const error = new Error(message || 'Unknown error') as NormalizedError
  if (typeof like?.code === 'string') error.code = like.code
  if (like && 'details' in like) error.details = like.details
  error.raw = value
  return error
}

export const getErrorMessage = (value: unknown): string => normalizeError(value).message
export const getErrorCode = (value: unknown): string | undefined => normalizeError(value).code

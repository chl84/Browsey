import { writable } from 'svelte/store'
import { detectNewFileType, type NewFileTypeMatch } from '../services/fileTypes'

const LOOKUP_DEBOUNCE_MS = 80

const formatHint = (match: NewFileTypeMatch | null) => {
  if (!match) return ''
  if (!match.matchedExt) return `Detected type: ${match.label}`
  return `Detected type: ${match.label} (.${match.matchedExt})`
}

export const createNewFileTypeHint = () => {
  const hint = writable('')
  let lookupTimer: ReturnType<typeof setTimeout> | null = null
  let requestId = 0
  let lastInput = ''

  const clearTimer = () => {
    if (lookupTimer !== null) {
      clearTimeout(lookupTimer)
      lookupTimer = null
    }
  }

  const reset = () => {
    clearTimer()
    requestId += 1
    lastInput = ''
    hint.set('')
  }

  const scheduleLookup = (name: string) => {
    const normalized = name.trim()
    if (normalized === lastInput) return
    lastInput = normalized
    clearTimer()
    if (!normalized) {
      requestId += 1
      hint.set('')
      return
    }

    const currentRequestId = ++requestId
    lookupTimer = setTimeout(() => {
      lookupTimer = null
      void detectNewFileType(normalized)
        .then((match) => {
          if (currentRequestId !== requestId) return
          hint.set(formatHint(match))
        })
        .catch(() => {
          if (currentRequestId !== requestId) return
          hint.set('')
        })
    }, LOOKUP_DEBOUNCE_MS)
  }

  return {
    hint,
    scheduleLookup,
    reset,
  }
}

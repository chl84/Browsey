import type { Entry } from '../types'
import type { AdvancedRenamePayload, SequenceMode } from './advancedRenameModal'

type RenamePreviewRow = {
  original: string
  next: string
}

type RenamePreview = {
  rows: RenamePreviewRow[]
  error: string
}

const splitExt = (name: string) => {
  const dot = name.lastIndexOf('.')
  if (dot <= 0 || dot === name.length - 1) return { stem: name, ext: '' }
  return { stem: name.slice(0, dot), ext: name.slice(dot) }
}

const toAlpha = (n: number) => {
  if (n < 0) return ''
  let num = Math.floor(n)
  let out = ''
  do {
    out = String.fromCharCode(65 + (num % 26)) + out
    num = Math.floor(num / 26) - 1
  } while (num >= 0)
  return out
}

const formatSequence = (
  index: number,
  mode: SequenceMode,
  start: number,
  step: number,
  padWidth: number,
) => {
  const pad = Math.max(0, Number(padWidth) || 0)
  const value = Number(start) + index * Number(step || 0)
  if (mode === 'numeric') {
    const num = Number.isFinite(value) ? Math.round(value) : 0
    return pad > 0 ? num.toString().padStart(pad, '0') : num.toString()
  }
  if (mode === 'alpha') {
    const alphaIndex = Math.max(0, Math.floor(value - 1))
    return toAlpha(alphaIndex)
  }
  return ''
}

export const computeAdvancedRenamePreview = (
  entries: Entry[],
  payload: AdvancedRenamePayload,
): RenamePreview => {
  let nextError = ''
  let pattern: RegExp | null = null
  const trimmed = payload.regex.trim()
  if (trimmed.length > 0) {
    try {
      pattern = new RegExp(trimmed, payload.caseSensitive ? 'g' : 'gi')
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      nextError = `Invalid regex: ${msg}`
      pattern = null
    }
  }

  const rows = entries.map((entry, idx) => {
    const seq =
      payload.sequenceMode !== 'none'
        ? formatSequence(
            idx,
            payload.sequenceMode,
            payload.sequenceStart,
            payload.sequenceStep,
            payload.sequencePad,
          )
        : ''
    const source = entry.name

    let base = source
    if (pattern) {
      base = source.replace(pattern, payload.replacement)
    } else if (!trimmed && payload.replacement) {
      base = payload.replacement
    }

    let next = base

    if (payload.prefix || payload.suffix) {
      const { stem, ext } = splitExt(next)
      next = `${payload.prefix}${stem}${payload.suffix}${ext}`
    }

    if (payload.sequenceMode !== 'none') {
      if (next.includes('$n')) {
        next = next.replaceAll('$n', seq)
      } else {
        const { stem, ext } = splitExt(next)
        next =
          payload.sequencePlacement === 'start'
            ? `${seq}${stem}${ext}`
            : `${stem}${seq}${ext}`
      }
    }

    return { original: source, next }
  })

  return { rows, error: nextError }
}

import { getErrorMessage } from '@/shared/lib/error'

type CopyResult = { ok: true } | { ok: false; error: string }

const fallbackCopyText = (value: string): CopyResult => {
  if (typeof document === 'undefined') {
    return { ok: false, error: 'Clipboard API is unavailable' }
  }
  try {
    const ta = document.createElement('textarea')
    ta.value = value
    ta.setAttribute('readonly', 'true')
    ta.style.position = 'fixed'
    ta.style.left = '-9999px'
    ta.style.top = '0'
    document.body.appendChild(ta)
    ta.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(ta)
    if (!ok) {
      return { ok: false, error: 'Copy command failed' }
    }
    return { ok: true }
  } catch (err) {
    return { ok: false, error: getErrorMessage(err) }
  }
}

export const copyTextToSystemClipboard = async (value: string): Promise<CopyResult> => {
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value)
      return { ok: true }
    } catch {
      // Fall through to legacy copy path below.
    }
  }
  return fallbackCopyText(value)
}

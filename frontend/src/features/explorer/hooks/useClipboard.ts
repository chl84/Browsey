import { clipboardState, setClipboardState, clearClipboardState } from '../stores/clipboardState'
import type { Entry } from '../types'
import { setClipboardCmd, pasteClipboardCmd } from '../services/clipboard'

type Result = { ok: true } | { ok: false; error: string }

const writeTextIfAvailable = async (value: string): Promise<Result> => {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) {
    return { ok: false, error: 'System clipboard is not available' }
  }
  try {
    await navigator.clipboard.writeText(value)
    return { ok: true }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return { ok: false, error: message }
  }
}

export const createClipboard = () => {
  const copy = async (entries: Entry[], opts: { writeText?: boolean } = {}): Promise<Result> => {
    if (entries.length === 0) return { ok: false, error: 'Nothing selected' }
    const paths = entries.map((e) => e.path)
    const writeTextResult: Result = opts.writeText
      ? await writeTextIfAvailable(paths.join('\n'))
      : { ok: true }
    try {
      await setClipboardCmd(paths, 'copy')
      setClipboardState('copy', entries)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return { ok: false, error: message }
    }
    if (!writeTextResult.ok) {
      return writeTextResult
    }
    return { ok: true }
  }

  const cut = async (entries: Entry[]): Promise<Result> => {
    if (entries.length === 0) return { ok: false, error: 'Nothing selected' }
    const paths = entries.map((e) => e.path)
    try {
      await setClipboardCmd(paths, 'cut')
      setClipboardState('cut', entries)
      return { ok: true }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return { ok: false, error: message }
    }
  }

  const paste = async (dest: string): Promise<Result> => {
    try {
      await pasteClipboardCmd(dest)
      return { ok: true }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return { ok: false, error: message }
    }
  }

  return {
    state: clipboardState,
    copy,
    cut,
    paste,
    clear: clearClipboardState,
  }
}

export type ClipboardApi = ReturnType<typeof createClipboard>

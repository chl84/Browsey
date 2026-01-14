import { invoke } from '@tauri-apps/api/core'
import { clipboardState, setClipboardState, clearClipboardState } from '../stores/clipboardState'
import type { Entry } from '../types'

type Result = { ok: true } | { ok: false; error: string }

const writeTextIfAvailable = async (value: string) => {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) return false
  try {
    await navigator.clipboard.writeText(value)
    return true
  } catch {
    return false
  }
}

export const createClipboard = () => {
  const copy = async (entries: Entry[], opts: { writeText?: boolean } = {}): Promise<Result> => {
    if (entries.length === 0) return { ok: false, error: 'Nothing selected' }
    const paths = entries.map((e) => e.path)
    try {
      await invoke('set_clipboard_cmd', { paths, mode: 'copy' })
      setClipboardState('copy', entries)
      if (opts.writeText) {
        await writeTextIfAvailable(paths.join('\n'))
      }
      return { ok: true }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return { ok: false, error: message }
    }
  }

  const cut = async (entries: Entry[]): Promise<Result> => {
    if (entries.length === 0) return { ok: false, error: 'Nothing selected' }
    const paths = entries.map((e) => e.path)
    try {
      await invoke('set_clipboard_cmd', { paths, mode: 'cut' })
      setClipboardState('cut', entries)
      return { ok: true }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return { ok: false, error: message }
    }
  }

  const paste = async (dest: string): Promise<Result> => {
    try {
      await invoke('paste_clipboard_cmd', { dest })
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

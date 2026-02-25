import { getErrorMessage } from '@/shared/lib/error'
import { clipboardState, setClipboardPathsState, clearClipboardState } from './clipboard.store'
import type { Entry } from '../model/types'
import { setClipboardCmd, pasteClipboardCmd } from '../services/clipboard.service'

type Result = { ok: true } | { ok: false; error: string }
type ClipboardMode = 'copy' | 'cut'

const isCloudPath = (path: string) => path.startsWith('rclone://')

const writeTextIfAvailable = async (value: string): Promise<Result> => {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) {
    return { ok: false, error: 'System clipboard is not available' }
  }
  try {
    await navigator.clipboard.writeText(value)
    return { ok: true }
  } catch (err) {
    const message = getErrorMessage(err)
    return { ok: false, error: message }
  }
}

export const createClipboard = () => {
  const setPaths = async (
    mode: ClipboardMode,
    paths: string[],
    opts: { writeText?: boolean } = {},
  ): Promise<Result> => {
    if (paths.length === 0) return { ok: false, error: 'Nothing selected' }
    const writeTextResult: Result = opts.writeText
      ? await writeTextIfAvailable(paths.join('\n'))
      : { ok: true }
    try {
      const cloudCount = paths.filter(isCloudPath).length
      if (cloudCount === paths.length) {
        // Cloud clipboard currently lives in frontend state; backend clipboard remains local-only.
      } else if (cloudCount > 0) {
        return { ok: false, error: 'Mixed local/cloud clipboard is not supported yet' }
      } else {
        await setClipboardCmd(paths, mode)
      }
      setClipboardPathsState(mode, paths)
    } catch (err) {
      const message = getErrorMessage(err)
      return { ok: false, error: message }
    }
    if (!writeTextResult.ok) {
      return writeTextResult
    }
    return { ok: true }
  }

  const copyPaths = async (paths: string[], opts: { writeText?: boolean } = {}): Promise<Result> =>
    setPaths('copy', paths, opts)

  const cutPaths = async (paths: string[]): Promise<Result> => setPaths('cut', paths)

  const copy = async (entries: Entry[], opts: { writeText?: boolean } = {}): Promise<Result> => {
    if (entries.length === 0) return { ok: false, error: 'Nothing selected' }
    const paths = entries.map((e) => e.path)
    return copyPaths(paths, opts)
  }

  const cut = async (entries: Entry[]): Promise<Result> => {
    if (entries.length === 0) return { ok: false, error: 'Nothing selected' }
    const paths = entries.map((e) => e.path)
    return cutPaths(paths)
  }

  const paste = async (dest: string): Promise<Result> => {
    try {
      await pasteClipboardCmd(dest)
      return { ok: true }
    } catch (err) {
      const message = getErrorMessage(err)
      return { ok: false, error: message }
    }
  }

  return {
    state: clipboardState,
    copy,
    copyPaths,
    cut,
    cutPaths,
    paste,
    clear: clearClipboardState,
  }
}

export type ClipboardApi = ReturnType<typeof createClipboard>

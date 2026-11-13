import { getErrorMessage } from '@/shared/lib/error'
import { copyPathsToSystemClipboard, getSystemClipboardPaths } from '../services/clipboard.service'

type ClipboardMode = 'copy' | 'cut'

export type SystemClipboardPaths = Readonly<{ mode: ClipboardMode; paths: readonly string[] }>

// The desktop clipboard is shared by every Browsey instance, while the internal
// clipboard belongs to this window. To tell a newer copy made elsewhere from a
// stale payload, remember what the desktop held the last time this instance
// wrote, imported or cleared it.
//   undefined: unknown, so nothing can be proven foreign and the internal clipboard wins.
//   null: the desktop clipboard held no file paths.
let lastSeen: SystemClipboardPaths | null | undefined
let pendingWrites: Promise<void> = Promise.resolve()
let writeGeneration = 0

const isNoPathsError = (error: unknown) =>
  getErrorMessage(error).toLowerCase().includes('no file paths found')

const toSnapshot = (
  content: { mode?: string; paths?: readonly string[] } | null | undefined,
): SystemClipboardPaths | null =>
  content && Array.isArray(content.paths) && content.paths.length > 0
    ? Object.freeze({
        mode: content.mode === 'cut' ? 'cut' : 'copy',
        paths: Object.freeze([...content.paths]),
      })
    : null

const sameContent = (a: SystemClipboardPaths | null, b: SystemClipboardPaths | null) => {
  if (a === null || b === null) return a === b
  if (a.mode !== b.mode || a.paths.length !== b.paths.length) return false
  const left = [...a.paths].sort()
  const right = [...b.paths].sort()
  return left.every((path, idx) => path === right[idx])
}

const observe = async (): Promise<SystemClipboardPaths | null | undefined> => {
  try {
    return toSnapshot(await getSystemClipboardPaths())
  } catch (error) {
    return isNoPathsError(error) ? null : undefined
  }
}

/**
 * Publishes paths to the desktop clipboard and resolves with the write itself.
 * Once it settles, the newest write records what the desktop holds: the read-back
 * form of our own paths, or the old payload a failed write left behind, which
 * must not later be mistaken for a newer copy from another application.
 */
export const writeSystemClipboard = (paths: string[], mode: ClipboardMode): Promise<void> => {
  const generation = ++writeGeneration
  const write = Promise.resolve(
    copyPathsToSystemClipboard(paths, mode === 'cut' ? 'cut' : undefined),
  )
  const recorded = write
    .catch(() => {})
    .then(async () => {
      const seen = await observe()
      if (generation === writeGeneration) lastSeen = seen
    })
  pendingWrites = Promise.all([pendingWrites, recorded]).then(() => {})
  return write
}

/** Records a payload this instance imported from the desktop clipboard. */
export const markSystemClipboardSeen = (content: SystemClipboardPaths) => {
  lastSeen = toSnapshot(content)
}

/** Records that this instance emptied the desktop clipboard. */
export const markSystemClipboardCleared = () => {
  lastSeen = null
}

/**
 * Returns the desktop clipboard only when it holds file paths that changed
 * since this instance last saw it, that is, a copy made in another Browsey
 * instance or another application. Pending writes from this instance are
 * awaited first so its own copy is never read back as foreign.
 */
export const foreignSystemClipboard = async (): Promise<SystemClipboardPaths | null> => {
  await pendingWrites
  if (lastSeen === undefined) return null
  let current: SystemClipboardPaths | null
  try {
    current = toSnapshot(await getSystemClipboardPaths())
  } catch {
    return null
  }
  return current && !sameContent(current, lastSeen) ? current : null
}

export const settleSystemClipboardWritesForTests = () => pendingWrites

export const resetSystemClipboardSyncForTests = () => {
  lastSeen = undefined
  pendingWrites = Promise.resolve()
  writeGeneration = 0
}

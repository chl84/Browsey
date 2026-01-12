import { writable } from 'svelte/store'
import type { Entry } from '../types'

type ClipboardMode = 'copy' | 'cut'
type ClipboardState = {
  mode: ClipboardMode
  paths: Set<string>
}

const initial: ClipboardState = { mode: 'copy', paths: new Set() }

export const clipboardState = writable<ClipboardState>(initial)

export const setClipboardState = (mode: ClipboardMode, entries: Entry[]) => {
  clipboardState.set({
    mode,
    paths: new Set(entries.map((e) => e.path)),
  })
}

export const clearClipboardState = () => {
  clipboardState.set(initial)
}

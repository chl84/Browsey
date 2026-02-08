import { writable } from 'svelte/store'
import type { Entry } from '../types'

type DuplicateCandidate = {
  path: string
  kind?: Entry['kind']
}

export type CheckDuplicatesState = {
  open: boolean
  target: Entry | null
  searchRoot: string
  duplicates: string[]
}

type Deps = {
  parentPath: (path: string) => string
}

export const createCheckDuplicatesModal = ({ parentPath }: Deps) => {
  const state = writable<CheckDuplicatesState>({
    open: false,
    target: null,
    searchRoot: '',
    duplicates: [],
  })

  const open = (entry: Entry) => {
    state.set({
      open: true,
      target: entry,
      searchRoot: parentPath(entry.path),
      duplicates: [],
    })
  }

  const close = () => {
    state.set({ open: false, target: null, searchRoot: '', duplicates: [] })
  }

  const setSearchRoot = (searchRoot: string) => {
    state.update((s) => ({ ...s, searchRoot }))
  }

  const setDuplicates = (candidates: DuplicateCandidate[]) => {
    const deduped = Array.from(
      new Set(
        candidates
          .filter((item) => item.kind !== 'link')
          .map((item) => item.path.trim())
          .filter((path) => path.length > 0),
      ),
    )
    state.update((s) => ({ ...s, duplicates: deduped }))
  }

  const setDuplicatePaths = (paths: string[]) => {
    setDuplicates(paths.map((path) => ({ path })))
  }

  return {
    state,
    open,
    close,
    setSearchRoot,
    setDuplicates,
    setDuplicatePaths,
  }
}

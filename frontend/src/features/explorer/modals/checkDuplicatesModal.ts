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
  scanning: boolean
  progressPercent: number
  progressLabel: string
  error: string
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
    scanning: false,
    progressPercent: 0,
    progressLabel: '',
    error: '',
  })

  const open = (entry: Entry) => {
    state.set({
      open: true,
      target: entry,
      searchRoot: parentPath(entry.path),
      duplicates: [],
      scanning: false,
      progressPercent: 0,
      progressLabel: '',
      error: '',
    })
  }

  const close = () => {
    state.set({
      open: false,
      target: null,
      searchRoot: '',
      duplicates: [],
      scanning: false,
      progressPercent: 0,
      progressLabel: '',
      error: '',
    })
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

  const startScan = (label = 'Scanning files...') => {
    state.update((s) => ({
      ...s,
      duplicates: [],
      scanning: true,
      progressPercent: 0,
      progressLabel: label,
      error: '',
    }))
  }

  const setProgress = (progressPercent: number, progressLabel: string) => {
    const clamped = Math.max(0, Math.min(100, Math.round(progressPercent)))
    state.update((s) => ({
      ...s,
      scanning: true,
      progressPercent: clamped,
      progressLabel,
    }))
  }

  const finishScan = (paths: string[]) => {
    const deduped = Array.from(
      new Set(
        paths
          .map((path) => path.trim())
          .filter((path) => path.length > 0),
      ),
    )
    state.update((s) => ({
      ...s,
      duplicates: deduped,
      scanning: false,
      progressPercent: 100,
      progressLabel: 'Scan complete',
      error: '',
    }))
  }

  const failScan = (error: string) => {
    state.update((s) => ({
      ...s,
      scanning: false,
      error,
      progressLabel: '',
    }))
  }

  const stopScan = () => {
    state.update((s) => ({ ...s, scanning: false }))
  }

  return {
    state,
    open,
    close,
    setSearchRoot,
    setDuplicates,
    setDuplicatePaths,
    startScan,
    setProgress,
    finishScan,
    failScan,
    stopScan,
  }
}

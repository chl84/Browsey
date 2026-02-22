import { listen } from '@tauri-apps/api/event'
import { get } from 'svelte/store'
import type { Writable } from 'svelte/store'
import type { Entry, ListingFacets, SortDirection, SortField } from '../model/types'
import { searchStream } from '../services/listing.service'
import type { ExplorerCallbacks } from './helpers'
import { appendEntries } from './entryMutations'
import {
  createSearchProgressEventName,
  mapSearchProgressChunkEntries,
  resolveSearchFinalEntries,
  type SearchProgressPayload,
} from './searchRuntimeHelpers'

type Deps = {
  entries: Writable<Entry[]>
  loading: Writable<boolean>
  error: Writable<string>
  filter: Writable<string>
  searchRunning: Writable<boolean>
  columnFacets: Writable<ListingFacets>
  current: Writable<string>
  clearFacetCache: () => void
  emptyListingFacets: () => ListingFacets
  onEntriesChanged?: ExplorerCallbacks['onEntriesChanged']
  getErrorMessage: (error: unknown) => string
  loadCurrentForEmptyNeedle: () => Promise<void>
  sortPayload: () => { field: SortField; direction: SortDirection }
  invalidateSearchRun: () => void
  getSearchRunId: () => number
  getCancelActiveSearch: () => (() => void) | null
  setCancelActiveSearch: (fn: (() => void) | null) => void
  getActiveSearchCancelId: () => string | null
  setActiveSearchCancelId: (id: string | null) => void
}

export const createSearchSession = (deps: Deps) => {
  return async (needleRaw: string) => {
    const needle = needleRaw.trim()
    const progressEvent = createSearchProgressEventName()
    deps.loading.set(true)
    deps.clearFacetCache()
    deps.error.set('')

    deps.invalidateSearchRun()
    const runId = deps.getSearchRunId()

    let buffer: Entry[] = []
    let raf: number | null = null
    let stop: (() => void) | null = null
    let cleaned = false

    const cleanup = () => {
      if (cleaned) return
      cleaned = true
      if (raf !== null) {
        cancelAnimationFrame(raf)
        raf = null
      }
      buffer = []
      if (stop) {
        stop()
        stop = null
      }
      if (deps.getCancelActiveSearch() === cleanup) {
        deps.setCancelActiveSearch(null)
      }
      if (deps.getActiveSearchCancelId() === progressEvent) {
        deps.setActiveSearchCancelId(null)
      }
    }

    deps.setCancelActiveSearch(cleanup)
    deps.setActiveSearchCancelId(progressEvent)

    if (needle.length === 0) {
      deps.searchRunning.set(false)
      await deps.loadCurrentForEmptyNeedle()
      if (runId === deps.getSearchRunId()) {
        deps.loading.set(false)
      }
      cleanup()
      return cleanup
    }

    deps.filter.set(needle)
    deps.searchRunning.set(true)
    deps.entries.set([])
    deps.onEntriesChanged?.()

    const flushBuffer = () => {
      if (runId !== deps.getSearchRunId()) {
        cleanup()
        return
      }
      if (buffer.length === 0) {
        raf = null
        return
      }
      deps.entries.update((list) => appendEntries(list, buffer))
      deps.onEntriesChanged?.()
      buffer = []
      raf = null
    }

    const scheduleFlush = () => {
      if (raf !== null) return
      raf = requestAnimationFrame(flushBuffer)
    }

    let unlisten: () => void
    try {
      unlisten = await listen<SearchProgressPayload>(progressEvent, (evt) => {
        if (runId !== deps.getSearchRunId()) {
          cleanup()
          return
        }
        if (evt.payload.error) {
          deps.error.set(evt.payload.error)
        }
        if (evt.payload.done) {
          if (raf !== null) {
            cancelAnimationFrame(raf)
            raf = null
          }
          const finalEntries = resolveSearchFinalEntries({
            payloadEntries: evt.payload.entries,
            currentEntries: get(deps.entries),
            bufferedEntries: buffer,
          })
          buffer = []
          // Keep streamed order on completion; sorting remains a manual UI action.
          deps.entries.set(finalEntries)
          if (evt.payload.facets) {
            deps.columnFacets.set(evt.payload.facets)
          }
          deps.onEntriesChanged?.()
          deps.searchRunning.set(false)
          deps.loading.set(false)
          cleanup()
          return
        }

        const chunkEntries = mapSearchProgressChunkEntries(evt.payload.entries)
        if (chunkEntries.length > 0) {
          buffer.push(...chunkEntries)
          scheduleFlush()
        }
      })
    } catch (err) {
      if (runId === deps.getSearchRunId()) {
        deps.error.set(deps.getErrorMessage(err))
        deps.searchRunning.set(false)
        deps.loading.set(false)
        deps.columnFacets.set(deps.emptyListingFacets())
      }
      cleanup()
      return cleanup
    }

    stop = unlisten
    if (cleaned) {
      stop()
      stop = null
      return cleanup
    }

    searchStream({
      path: get(deps.current),
      query: needle,
      sort: deps.sortPayload(),
      progressEvent,
    }).catch((err) => {
      if (runId !== deps.getSearchRunId()) {
        cleanup()
        return
      }
      deps.error.set(deps.getErrorMessage(err))
      deps.searchRunning.set(false)
      deps.loading.set(false)
      deps.columnFacets.set(deps.emptyListingFacets())
      cleanup()
    })

    return cleanup
  }
}


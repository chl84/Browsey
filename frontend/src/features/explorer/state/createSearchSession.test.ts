import { describe, expect, it, vi, beforeEach } from 'vitest'
import { get, writable } from 'svelte/store'
import { createSearchSession } from './createSearchSession'
import type { ListingFacets } from '../model/types'

const listenMock = vi.fn()
const searchStreamMock = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}))

vi.mock('../services/listing.service', () => ({
  searchStream: (...args: unknown[]) => searchStreamMock(...args),
}))

const emptyFacets = (): ListingFacets => ({
  name: [],
  type: [],
  modified: [],
  size: [],
})

const makeDeps = () => {
  const entries = writable([])
  const loading = writable(false)
  const error = writable('')
  const filter = writable('')
  const searchRunning = writable(false)
  const columnFacets = writable<ListingFacets>({
    name: [{ id: 'stale', label: 'stale' }],
    type: [],
    modified: [],
    size: [],
  })
  const current = writable('/tmp')

  let searchRunId = 0
  let cancelActiveSearch: (() => void) | null = null
  let activeSearchCancelId: string | null = null

  return {
    deps: {
      entries,
      loading,
      error,
      filter,
      searchRunning,
      columnFacets,
      current,
      clearFacetCache: vi.fn(),
      emptyListingFacets: emptyFacets,
      onEntriesChanged: vi.fn(),
      getErrorMessage: (value: unknown) => (value instanceof Error ? value.message : String(value)),
      loadCurrentForEmptyNeedle: vi.fn(async () => {}),
      sortPayload: () => ({ field: 'name' as const, direction: 'asc' as const }),
      invalidateSearchRun: () => {
        searchRunId += 1
      },
      getSearchRunId: () => searchRunId,
      getCancelActiveSearch: () => cancelActiveSearch,
      setCancelActiveSearch: (fn: (() => void) | null) => {
        cancelActiveSearch = fn
      },
      getActiveSearchCancelId: () => activeSearchCancelId,
      setActiveSearchCancelId: (id: string | null) => {
        activeSearchCancelId = id
      },
    },
    stores: { entries, loading, error, filter, searchRunning, columnFacets },
    getCancelActiveSearch: () => cancelActiveSearch,
    getActiveSearchCancelId: () => activeSearchCancelId,
  }
}

describe('createSearchSession recovery', () => {
  beforeEach(() => {
    listenMock.mockReset()
    searchStreamMock.mockReset()
  })

  it('surfaces listener setup failure without leaving search running', async () => {
    listenMock.mockRejectedValueOnce(new Error('listen failed'))
    const { deps, stores, getCancelActiveSearch, getActiveSearchCancelId } = makeDeps()
    const runSearch = createSearchSession(deps)

    await runSearch('invoice')

    expect(get(stores.error)).toBe('listen failed')
    expect(get(stores.searchRunning)).toBe(false)
    expect(get(stores.loading)).toBe(false)
    expect(get(stores.columnFacets)).toEqual(emptyFacets())
    expect(getCancelActiveSearch()).toBeNull()
    expect(getActiveSearchCancelId()).toBeNull()
  })

  it('surfaces backend search failure without leaving stale running state', async () => {
    listenMock.mockResolvedValueOnce(() => {})
    searchStreamMock.mockRejectedValueOnce(new Error('backend search failed'))
    const { deps, stores, getCancelActiveSearch, getActiveSearchCancelId } = makeDeps()
    const runSearch = createSearchSession(deps)

    await runSearch('invoice')
    await Promise.resolve()

    expect(get(stores.error)).toBe('backend search failed')
    expect(get(stores.searchRunning)).toBe(false)
    expect(get(stores.loading)).toBe(false)
    expect(get(stores.columnFacets)).toEqual(emptyFacets())
    expect(getCancelActiveSearch()).toBeNull()
    expect(getActiveSearchCancelId()).toBeNull()
  })
})

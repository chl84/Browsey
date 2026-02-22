import type { Entry, ListingFacets } from '../model/types'
import { appendEntries } from './entryMutations'
import { mapNameLower } from './helpers'

export type SearchProgressPayload = {
  entries: Entry[]
  done: boolean
  error?: string
  facets?: ListingFacets
}

export const createSearchProgressEventName = () =>
  `search-progress-${Math.random().toString(16).slice(2)}`

export const mapSearchProgressChunkEntries = (entries?: Entry[] | null): Entry[] =>
  entries && entries.length > 0 ? mapNameLower(entries) : []

export const resolveSearchFinalEntries = (params: {
  payloadEntries?: Entry[] | null
  currentEntries: Entry[]
  bufferedEntries: Entry[]
}): Entry[] => {
  const doneEntries = mapSearchProgressChunkEntries(params.payloadEntries)
  if (doneEntries.length > 0) {
    return doneEntries
  }
  return appendEntries(params.currentEntries, params.bufferedEntries)
}


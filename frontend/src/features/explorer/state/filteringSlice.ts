import { derived, get, writable } from 'svelte/store'
import type { Entry, SortField } from '../model/types'
import { listFacets, type FacetScope } from '../services/listing.service'
import { modifiedBucket, sizeBucket, typeLabel } from '../filters/columnBuckets'
import { nameBucket } from '../filters/nameFilters'
import { FILTER_DEBOUNCE_MS, emptyListingFacets } from './helpers'
import type { ColumnFilters } from './helpers'
import type { ExplorerStores } from './stores'

const applyFoldersFirst = (list: Entry[], foldersFirstOn: boolean) => {
  if (!foldersFirstOn) return list
  const dirs: Entry[] = []
  const rest: Entry[] = []
  for (const e of list) {
    if (e.kind === 'dir') {
      dirs.push(e)
    } else {
      rest.push(e)
    }
  }
  return [...dirs, ...rest]
}

const applyColumnFilters = (list: Entry[], filters: ColumnFilters) => {
  const hasName = filters.name.size > 0
  const hasType = filters.type.size > 0
  const hasModified = filters.modified.size > 0
  const hasSize = filters.size.size > 0
  if (!hasName && !hasType && !hasModified && !hasSize) return list

  return list.filter((e) => {
    if (hasName) {
      const bucket = nameBucket(e.nameLower ?? e.name.toLowerCase())
      if (!filters.name.has(bucket)) return false
    }

    if (hasType) {
      const label = typeLabel(e)
      const id = `type:${label}`
      if (!filters.type.has(id)) return false
    }

    if (hasModified) {
      const bucket = modifiedBucket(e.modified)
      if (!bucket) return false
      const id = `modified:${bucket.label}`
      if (!filters.modified.has(id)) return false
    }

    if (hasSize) {
      if (e.kind !== 'file') return false
      if (typeof e.size !== 'number') return false
      const bucket = sizeBucket(e.size)
      if (!bucket) return false
      const id = `size:${bucket.label}`
      if (!filters.size.has(id)) return false
    }

    return true
  })
}

const currentFacetScope = (where: string): { scope: FacetScope; path?: string } | null => {
  if (where === 'Recent') return { scope: 'recent' }
  if (where === 'Starred') return { scope: 'starred' }
  if (where === 'Network') return null
  if (where === 'Trash') return { scope: 'trash' }
  if (where.trim().length === 0) return null
  return { scope: 'dir', path: where }
}

const facetCacheKey = (
  context: { scope: FacetScope; path?: string },
  includeHidden: boolean,
): string => `${context.scope}|${context.path ?? ''}|hidden:${includeHidden ? 1 : 0}`

export const createFilteringSlice = (stores: Pick<
  ExplorerStores,
  'entries' | 'showHidden' | 'hiddenFilesLast' | 'foldersFirst' | 'filter' | 'searchMode' | 'current'
>) => {
  const { entries, showHidden, hiddenFilesLast, foldersFirst, filter, searchMode, current } = stores

  const visibleEntries = derived(
    [entries, showHidden, hiddenFilesLast, foldersFirst],
    ([$entries, $showHidden, $hiddenLast, $foldersFirst]) => {
      const base = $showHidden
        ? $entries
        : $entries.filter((e) => !(e.hidden === true || e.name.startsWith('.')))
      if ($showHidden && $hiddenLast) {
        const hiddenList: Entry[] = []
        const visibleList: Entry[] = []
        for (const e of base) {
          if (e.hidden === true || e.name.startsWith('.')) {
            hiddenList.push(e)
          } else {
            visibleList.push(e)
          }
        }
        return [
          ...applyFoldersFirst(visibleList, $foldersFirst),
          ...applyFoldersFirst(hiddenList, $foldersFirst),
        ]
      }
      return applyFoldersFirst(base, $foldersFirst)
    },
  )

  const columnFilters = writable<ColumnFilters>({
    name: new Set(),
    type: new Set(),
    modified: new Set(),
    size: new Set(),
  })
  const columnFacets = writable(emptyListingFacets())
  const columnFacetsLoading = writable(false)
  const facetCache = new Map<string, ReturnType<typeof emptyListingFacets>>()
  let facetRequestSeq = 0

  const invalidateFacetCache = () => {
    facetRequestSeq += 1
    facetCache.clear()
    columnFacetsLoading.set(false)
  }

  const clearFacetCache = () => {
    invalidateFacetCache()
    columnFacets.set(emptyListingFacets())
  }

  const ensureColumnFacets = async () => {
    if (get(searchMode)) {
      return
    }
    const context = currentFacetScope(get(current))
    if (!context) {
      columnFacets.set(emptyListingFacets())
      return
    }

    const includeHidden = get(showHidden)
    const key = facetCacheKey(context, includeHidden)
    const cached = facetCache.get(key)
    if (cached) {
      columnFacets.set(cached)
      return
    }

    const req = ++facetRequestSeq
    columnFacetsLoading.set(true)
    try {
      const facets = await listFacets({
        scope: context.scope,
        path: context.path,
        includeHidden,
      })
      if (req !== facetRequestSeq) {
        return
      }
      facetCache.set(key, facets)
      const latest = currentFacetScope(get(current))
      const latestKey = latest ? facetCacheKey(latest, get(showHidden)) : null
      if (latestKey === key) {
        columnFacets.set(facets)
      }
    } catch (err) {
      console.error('Failed to load column facets', err)
      if (req === facetRequestSeq) {
        columnFacets.set(emptyListingFacets())
      }
    } finally {
      if (req === facetRequestSeq) {
        columnFacetsLoading.set(false)
      }
    }
  }

  const filteredEntries = derived(
    [visibleEntries, filter, columnFilters, searchMode],
    ([$visible, $filter, $filters, $searchMode], set) => {
      let timer: ReturnType<typeof setTimeout> | undefined
      const needle = $filter.trim().toLowerCase()

      const compute = () => {
        const applyTextFilter = needle.length > 0 && !$searchMode
        let base =
          !applyTextFilter
            ? $visible
            : $visible.filter((e) => (e.nameLower ?? e.name.toLowerCase()).includes(needle))
        base = applyColumnFilters(base, $filters)
        set(base)
      }

      const shouldDebounce = needle.length > 0 && !$searchMode
      if (shouldDebounce) {
        timer = setTimeout(compute, FILTER_DEBOUNCE_MS)
      } else {
        compute()
      }

      return () => {
        if (timer) {
          clearTimeout(timer)
        }
      }
    },
    [] as Entry[],
  )

  const resetColumnFilter = (field: SortField) => {
    if (field !== 'name' && field !== 'type' && field !== 'modified' && field !== 'size') return
    columnFilters.update((f) => ({
      ...f,
      [field]: new Set<string>(),
    }))
  }

  const toggleColumnFilter = (field: SortField, id: string, checked: boolean) => {
    if (field !== 'name' && field !== 'type' && field !== 'modified' && field !== 'size') return
    columnFilters.update((f) => {
      const next = {
        ...f,
        [field]: new Set(f[field]),
      }
      if (checked) {
        next[field].add(id)
      } else {
        next[field].delete(id)
      }
      return next
    })
  }

  return {
    visibleEntries,
    filteredEntries,
    columnFilters,
    columnFacets,
    columnFacetsLoading,
    invalidateFacetCache,
    clearFacetCache,
    ensureColumnFacets,
    resetColumnFilter,
    toggleColumnFilter,
  }
}

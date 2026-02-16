<script lang="ts">
  import ColumnFilterMenu from './ColumnFilterMenu.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import type { Entry, FilterOption, ListingFacets, SortField } from '../types'
  import { nameBucket, nameFilterLabel, nameFilterRank } from '../filters/nameFilters'
  import { modifiedBucket, modifiedFilterRank, sizeBucket, sizeFilterRank, typeLabel } from '../filters/columnBuckets'

  export let loading = false
  export let filterValue = ''
  export let filterSourceEntries: Entry[] = []
  export let columnFilters: {
    name: Set<string>
    type: Set<string>
    modified: Set<string>
    size: Set<string>
  } = { name: new Set(), type: new Set(), modified: new Set(), size: new Set() }
  export let columnFacets: ListingFacets = { name: [], type: [], modified: [], size: [] }
  export let columnFacetsLoading = false
  export let onEnsureColumnFacets: () => void | Promise<void> = () => {}
  export let onToggleFilter: (field: SortField, id: string, checked: boolean) => void = () => {}
  export let onResetFilter: (field: SortField) => void = () => {}
  export let filterActive: Record<SortField, boolean> = {
    name: false,
    type: false,
    modified: false,
    size: false,
    starred: false,
  }

  let filterMenuOpen = false
  let filterMenuAnchor: DOMRect | null = null
  let filterMenuField: SortField | null = null
  let filterMenuOptions: FilterOption[] = []
  let filterMenuPendingLoad = false
  let filterMenuLoadSeq = 0
  let filterMenuLoadAttempts = 0
  let filterCtxOpen = false
  let filterCtxX = 0
  let filterCtxY = 0
  let filterCtxField: SortField | null = null

  $: activeNameFilters = columnFilters.name
  $: activeTypeFilters = columnFilters.type
  $: activeModifiedFilters = columnFilters.modified
  $: activeSizeFilters = columnFilters.size

  type FilterField = 'name' | 'type' | 'modified' | 'size'

  const isFilterField = (field: SortField): field is FilterField =>
    field === 'name' || field === 'type' || field === 'modified' || field === 'size'

  const facetOptionsForField = (field: FilterField): FilterOption[] => {
    if (field === 'name') return columnFacets.name
    if (field === 'type') return columnFacets.type
    if (field === 'modified') return columnFacets.modified
    return columnFacets.size
  }

  const selectedForField = (field: FilterField): Set<string> => {
    if (field === 'name') return activeNameFilters
    if (field === 'type') return activeTypeFilters
    if (field === 'modified') return activeModifiedFilters
    return activeSizeFilters
  }

  const entryMatchesFilter = (entry: Entry, field: FilterField): boolean => {
    if (field === 'name') {
      const bucket = nameBucket(entry.nameLower ?? entry.name.toLowerCase())
      return activeNameFilters.has(bucket)
    }
    if (field === 'type') {
      return activeTypeFilters.has(`type:${typeLabel(entry)}`)
    }
    if (field === 'modified') {
      const bucket = modifiedBucket(entry.modified)
      return !!bucket && activeModifiedFilters.has(`modified:${bucket.label}`)
    }
    if (entry.kind !== 'file' || typeof entry.size !== 'number') return false
    const bucket = sizeBucket(entry.size)
    return !!bucket && activeSizeFilters.has(`size:${bucket.label}`)
  }

  const entriesForFilterField = (field: FilterField): Entry[] => {
    const needle = filterValue.trim().toLowerCase()
    const base =
      needle.length === 0
        ? filterSourceEntries
        : filterSourceEntries.filter((e) => (e.nameLower ?? e.name.toLowerCase()).includes(needle))
    return base.filter((entry) => {
      if (field !== 'name' && activeNameFilters.size > 0 && !entryMatchesFilter(entry, 'name')) return false
      if (field !== 'type' && activeTypeFilters.size > 0 && !entryMatchesFilter(entry, 'type')) return false
      if (field !== 'modified' && activeModifiedFilters.size > 0 && !entryMatchesFilter(entry, 'modified')) return false
      if (field !== 'size' && activeSizeFilters.size > 0 && !entryMatchesFilter(entry, 'size')) return false
      return true
    })
  }

  const availableOptionIds = (field: FilterField): Set<string> => {
    const set = new Set<string>()
    const entries = entriesForFilterField(field)
    for (const entry of entries) {
      if (field === 'name') {
        set.add(nameBucket(entry.nameLower ?? entry.name.toLowerCase()))
        continue
      }
      if (field === 'type') {
        set.add(`type:${typeLabel(entry)}`)
        continue
      }
      if (field === 'modified') {
        const bucket = modifiedBucket(entry.modified)
        if (bucket) {
          set.add(`modified:${bucket.label}`)
        }
        continue
      }
      if (entry.kind !== 'file' || typeof entry.size !== 'number') continue
      const bucket = sizeBucket(entry.size)
      if (bucket) {
        set.add(`size:${bucket.label}`)
      }
    }
    return set
  }

  const filterOptionsForField = (field: FilterField): FilterOption[] => {
    const selected = selectedForField(field)
    const available = availableOptionIds(field)
    const wantedIds = new Set<string>([...available, ...selected])
    if (wantedIds.size === 0) return []

    const facetMap = new Map<string, FilterOption>()
    for (const opt of facetOptionsForField(field)) {
      facetMap.set(opt.id, opt)
    }

    const options: FilterOption[] = []
    for (const id of wantedIds) {
      const fromFacets = facetMap.get(id)
      if (fromFacets) {
        options.push(fromFacets)
        continue
      }
      const fallbackLabel =
        field === 'name'
          ? nameFilterLabel(id)
          : id.startsWith(`${field}:`)
            ? id.slice(field.length + 1)
            : id
      options.push({ id, label: fallbackLabel })
    }

    options.sort((a, b) => {
      if (field === 'name') {
        const rankDiff = nameFilterRank(a.id) - nameFilterRank(b.id)
        if (rankDiff !== 0) return rankDiff
      } else if (field === 'modified') {
        const rankDiff = modifiedFilterRank(a.id) - modifiedFilterRank(b.id)
        if (rankDiff !== 0) return rankDiff
      } else if (field === 'size') {
        const rankDiff = sizeFilterRank(a.id) - sizeFilterRank(b.id)
        if (rankDiff !== 0) return rankDiff
      }
      return a.label.localeCompare(b.label, undefined, { sensitivity: 'base' })
    })

    return options
  }

  const requestFilterFacets = () => {
    const req = ++filterMenuLoadSeq
    filterMenuLoadAttempts += 1
    filterMenuPendingLoad = true
    Promise.resolve(onEnsureColumnFacets())
      .catch(() => {})
      .finally(() => {
        if (req === filterMenuLoadSeq) {
          filterMenuPendingLoad = false
        }
      })
  }

  export function handleFilterClick(field: SortField, anchor: DOMRect) {
    if (!isFilterField(field)) {
      return
    }
    filterMenuLoadAttempts = 0
    filterMenuOpen = true
    filterMenuAnchor = anchor
    filterMenuField = field
    filterMenuOptions = filterOptionsForField(field)
    if (filterMenuOptions.length === 0) {
      requestFilterFacets()
    } else {
      filterMenuPendingLoad = false
    }
  }

  const closeFilterMenu = () => {
    filterMenuLoadSeq += 1
    filterMenuLoadAttempts = 0
    filterMenuPendingLoad = false
    filterMenuOpen = false
  }

  export function handleFilterContextMenu(field: SortField, event: MouseEvent) {
    event.preventDefault()
    filterMenuOpen = false
    filterCtxField = field
    filterCtxX = event.clientX
    filterCtxY = event.clientY
    filterCtxOpen = true
  }

  const closeFilterContextMenu = () => {
    filterCtxOpen = false
    filterCtxField = null
  }

  const handleFilterContextSelect = (id: string) => {
    if (id !== 'reset' || !filterCtxField) {
      closeFilterContextMenu()
      return
    }
    onResetFilter(filterCtxField)
    closeFilterContextMenu()
  }

  $: filterActive = {
    name: activeNameFilters.size > 0,
    type: activeTypeFilters.size > 0,
    modified: activeModifiedFilters.size > 0,
    size: activeSizeFilters.size > 0,
    starred: false,
  }

  $: if (filterMenuOpen && filterMenuField && isFilterField(filterMenuField)) {
    // Keep these dependencies explicit so menu options recompute while popup is open.
    const deps = [
      filterValue,
      filterSourceEntries,
      columnFacets.name,
      columnFacets.type,
      columnFacets.modified,
      columnFacets.size,
      activeNameFilters,
      activeTypeFilters,
      activeModifiedFilters,
      activeSizeFilters,
    ]
    void deps
    filterMenuOptions = filterOptionsForField(filterMenuField)
  }

  $: if (filterMenuOpen && filterMenuOptions.length > 0 && filterMenuPendingLoad) {
    filterMenuPendingLoad = false
  }

  $: if (
    filterMenuOpen &&
    filterMenuField &&
    isFilterField(filterMenuField) &&
    filterMenuOptions.length === 0 &&
    !loading &&
    !columnFacetsLoading &&
    !filterMenuPendingLoad &&
    filterMenuLoadAttempts < 2
  ) {
    requestFilterFacets()
  }
</script>

<ColumnFilterMenu
  open={filterMenuOpen}
  loading={
    loading ||
    columnFacetsLoading ||
    filterMenuPendingLoad ||
    (filterMenuOpen && filterMenuOptions.length === 0 && filterMenuLoadAttempts < 2)
  }
  options={filterMenuOptions}
  selected={
    filterMenuField === 'type'
      ? activeTypeFilters
      : filterMenuField === 'modified'
        ? activeModifiedFilters
        : filterMenuField === 'size'
          ? activeSizeFilters
          : activeNameFilters
  }
  anchor={filterMenuAnchor}
  onToggle={(id, checked) => {
    if (!filterMenuField || !isFilterField(filterMenuField)) return
    onToggleFilter(filterMenuField, id, checked)
  }}
  onClose={closeFilterMenu}
/>

<ContextMenu
  open={filterCtxOpen}
  x={filterCtxX}
  y={filterCtxY}
  actions={[{ id: 'reset', label: 'Reset' }]}
  onSelect={handleFilterContextSelect}
  onClose={closeFilterContextMenu}
/>

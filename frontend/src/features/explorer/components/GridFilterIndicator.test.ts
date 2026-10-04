import { mount, tick, unmount } from 'svelte'
import { get, writable } from 'svelte/store'
import { afterEach, expect, it, vi } from 'vitest'
import GridFilterIndicator from './GridFilterIndicator.svelte'
import { createFilteringSlice } from '../state/filteringSlice'
import type { ColumnFilters } from '../state/helpers'
import type { Entry } from '../model/types'

const components: ReturnType<typeof mount>[] = []
const emptyFilters = (): ColumnFilters => ({
  name: new Set(), type: new Set(), modified: new Set(), size: new Set(),
})

afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

it('does not show an indicator or reset button without column filters', async () => {
  components.push(mount(GridFilterIndicator, {
    target: document.body, props: { columnFilters: emptyFilters() },
  }))
  await tick()
  expect(document.querySelector('.grid-filter-indicator')).toBeNull()
  expect(document.querySelector('button')).toBeNull()
})

it.each(['name', 'type', 'modified', 'size'] as const)(
  'shows Reset for an active %s filter and resets all four column fields', async (field) => {
    const columnFilters = emptyFilters()
    columnFilters[field].add('active')
    const onResetFilter = vi.fn()
    components.push(mount(GridFilterIndicator, {
      target: document.body, props: { columnFilters, onResetFilter },
    }))
    await tick()
    expect(document.querySelector('.grid-filter-indicator')?.textContent).toContain('Column filters active')
    const button = document.querySelector<HTMLButtonElement>('button')!
    expect(button.textContent).toBe('Reset')
    expect(button.type).toBe('button')
    expect(button.getAttribute('aria-label')).toBe('Reset column filters')
    button.click()
    expect(onResetFilter.mock.calls).toEqual([['name'], ['type'], ['modified'], ['size']])
  },
)

it.each([false, true])('preserves text and search state when resetting columns (searchMode=%s)', async (searchMode) => {
  const entries: Entry[] = [
    { name: 'notes.txt', path: '/mock/notes.txt', kind: 'file', size: 10, modified: null, iconId: 0 },
    { name: 'photo.png', path: '/mock/photo.png', kind: 'file', size: 20, modified: null, iconId: 0 },
  ]
  const filter = writable('notes')
  const searching = writable(searchMode)
  const current = writable('/mock')
  const showHidden = writable(false)
  const slice = createFilteringSlice({
    entries: writable(entries), filter, searchMode: searching, current, showHidden,
    hiddenFilesLast: writable(false), foldersFirst: writable(true),
  })
  slice.columnFilters.set({
    name: new Set(['other']), type: new Set(['type:Other']),
    modified: new Set(['modified:Other']), size: new Set(['size:Other']),
  })
  components.push(mount(GridFilterIndicator, {
    target: document.body,
    props: { columnFilters: get(slice.columnFilters), onResetFilter: slice.resetColumnFilter },
  }))
  await tick()
  document.querySelector<HTMLButtonElement>('button')!.click()
  expect(get(slice.columnFilters)).toEqual(emptyFilters())
  expect(get(filter)).toBe('notes')
  expect(get(searching)).toBe(searchMode)
  expect(get(current)).toBe('/mock')
  expect(get(showHidden)).toBe(false)
})

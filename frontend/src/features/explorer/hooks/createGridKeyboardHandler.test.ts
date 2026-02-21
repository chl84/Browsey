import { describe, expect, it, vi } from 'vitest'
import { get, writable } from 'svelte/store'
import { createGridKeyboardHandler } from './createGridKeyboardHandler'
import type { Entry } from '../model/types'

const entry = (path: string): Entry => ({
  name: path,
  path,
  kind: 'file',
  iconId: 0,
})

describe('createGridKeyboardHandler', () => {
  it('clears selection on Escape', () => {
    const selected = writable(new Set<string>(['a', 'b']))
    const anchorIndex = writable<number | null>(1)
    const caretIndex = writable<number | null>(2)
    const handler = createGridKeyboardHandler({
      getFilteredEntries: () => [entry('a'), entry('b'), entry('c')],
      selected,
      anchorIndex,
      caretIndex,
      getGridCols: () => 2,
      ensureGridVisible: vi.fn(),
      handleOpenEntry: vi.fn(),
    })

    const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true })
    handler(event)

    expect(get(selected)).toEqual(new Set())
    expect(get(anchorIndex)).toBeNull()
    expect(get(caretIndex)).toBeNull()
    expect(event.defaultPrevented).toBe(true)
  })

  it('extends selection range with Shift+Arrow', () => {
    const selected = writable(new Set<string>(['a']))
    const anchorIndex = writable<number | null>(0)
    const caretIndex = writable<number | null>(0)
    const ensureGridVisible = vi.fn()
    const handler = createGridKeyboardHandler({
      getFilteredEntries: () => [entry('a'), entry('b'), entry('c'), entry('d')],
      selected,
      anchorIndex,
      caretIndex,
      getGridCols: () => 2,
      ensureGridVisible,
      handleOpenEntry: vi.fn(),
    })

    const event = new KeyboardEvent('keydown', { key: 'ArrowDown', shiftKey: true })
    handler(event)

    expect(get(selected)).toEqual(new Set(['a', 'b', 'c']))
    expect(get(anchorIndex)).toBe(0)
    expect(get(caretIndex)).toBe(2)
    expect(ensureGridVisible).toHaveBeenCalledWith(2)
  })

  it('opens current entry on Enter', () => {
    const selected = writable(new Set<string>(['b']))
    const anchorIndex = writable<number | null>(1)
    const caretIndex = writable<number | null>(1)
    const handleOpenEntry = vi.fn()
    const handler = createGridKeyboardHandler({
      getFilteredEntries: () => [entry('a'), entry('b'), entry('c')],
      selected,
      anchorIndex,
      caretIndex,
      getGridCols: () => 2,
      ensureGridVisible: vi.fn(),
      handleOpenEntry,
    })

    handler(new KeyboardEvent('keydown', { key: 'Enter' }))

    expect(handleOpenEntry).toHaveBeenCalledTimes(1)
    expect(handleOpenEntry).toHaveBeenCalledWith(expect.objectContaining({ path: 'b' }))
  })
})

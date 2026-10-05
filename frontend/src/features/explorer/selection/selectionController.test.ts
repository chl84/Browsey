import { expect, it } from 'vitest'
import { applyClickSelection } from './selectionController'
import type { Entry } from '../model/types'
const entries: Entry[] = ['file', 'folder'].map((name, i) => ({ name, path: `/owned/${name}`, kind: i === 0 ? 'file' : 'dir', iconId: 0 }))
it.each([1, 2])('keeps the complete selection, anchor and caret on button %s', button => {
  const previous = { selected: new Set(entries.map(e => e.path)), anchor: 0, caret: 1 }
  for (const modifiers of [{}, { ctrlKey: true }, { shiftKey: true }]) {
    const next = applyClickSelection(entries, 0, new MouseEvent('click', { button, ...modifiers }), previous)
    expect(next).toBe(previous)
  }
})
it('retains primary-click single, toggle and range selection', () => {
  const previous = { selected: new Set([entries[0].path]), anchor: 0, caret: 0 }
  expect([...applyClickSelection(entries, 1, new MouseEvent('click'), previous).selected]).toEqual([entries[1].path])
  expect([...applyClickSelection(entries, 1, new MouseEvent('click', { ctrlKey: true }), previous).selected]).toEqual(entries.map(e => e.path))
  expect([...applyClickSelection(entries, 1, new MouseEvent('click', { shiftKey: true }), previous).selected]).toEqual(entries.map(e => e.path))
})

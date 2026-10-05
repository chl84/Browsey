import { expect, it } from 'vitest'
import { createPrimaryClickGuard } from './primaryClick'

it('ignores a misreported primary click from a context gesture without blocking the next real click', () => {
  const guard = createPrimaryClickGuard()
  guard.down(new MouseEvent('mousedown', { button: 2 }))
  expect(guard.accept(new MouseEvent('click', { button: 0, detail: 1 }))).toBe(false)
  guard.down(new MouseEvent('mousedown', { button: 0 }))
  expect(guard.accept(new MouseEvent('click', { button: 0, detail: 1 }))).toBe(true)
})
it('keeps keyboard activation and ordinary non-primary rejection', () => {
  const guard = createPrimaryClickGuard()
  guard.down(new MouseEvent('mousedown', { button: 2 }))
  expect(guard.accept(new MouseEvent('click', { detail: 0 }))).toBe(true)
  expect(guard.accept(new MouseEvent('click', { button: 2, detail: 1 }))).toBe(false)
})

import { expect, it } from 'vitest'
import { formatSize } from './formatSize'

it.each([
  [undefined, ''], [null, ''], [0, '0 B'], [1000, '1000 B'],
  [1024, '1.0 kB'], [8192, '8.2 kB'], [1_000_000, '1.0 MB'],
])('retains existing file-size formatting for %s', (bytes, expected) => {
  expect(formatSize(bytes)).toBe(expected)
})

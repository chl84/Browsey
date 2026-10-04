import { expect, it } from 'vitest'
import { partitionCapacity } from './partitionCapacity'

it.each([undefined, null, 0, -1, NaN, Infinity])('hides unknown or invalid capacity: %s', (bytes) => {
  expect(partitionCapacity(bytes)).toBe('')
})

it.each([
  [32_000_000_000, '32 GB'],
  [31_999_000_000, '32 GB'],
  [512_000_000_000, '512 GB'],
  [1_000_000_000_000, '1,000 GB'],
  [512_000_000, '0.5 GB'],
  [50_000_000, '<0.1 GB'],
])('formats %s bytes in decimal GB', (bytes, expected) => {
  expect(partitionCapacity(bytes)).toBe(expected)
})

import { describe, expect, it } from 'vitest'
import type { Entry } from '../model/types'
import { typeLabel } from './columnBuckets'

const makeEntry = (overrides: Partial<Entry>): Entry => ({
  name: 'item',
  path: '/tmp/item',
  kind: 'file',
  iconId: 0,
  ...overrides,
})

describe('typeLabel', () => {
  it('returns dir for directories even when ext is present', () => {
    const entry = makeEntry({
      name: 'folder.with.dot',
      path: '/tmp/folder.with.dot',
      kind: 'dir',
      ext: 'dot',
    })
    expect(typeLabel(entry)).toBe('dir')
  })

  it('returns lowercase ext for files', () => {
    const entry = makeEntry({
      name: 'Report.PDF',
      path: '/tmp/Report.PDF',
      kind: 'file',
      ext: 'PDF',
    })
    expect(typeLabel(entry)).toBe('pdf')
  })
})

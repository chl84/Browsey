import { describe, expect, it } from 'vitest'
import { filterSidebarEntries } from './sidebarFilter'

describe('filterSidebarEntries', () => {
  it('returns all entries when the query is empty', () => {
    const result = filterSidebarEntries({
      query: '',
      places: [{ label: 'Home', path: '~' }],
      bookmarks: [{ label: 'Docs', path: '/tmp/docs' }],
      partitions: [{ label: 'Data', path: '/mnt/data', removable: false }],
    })

    expect(result.places).toHaveLength(1)
    expect(result.bookmarks).toHaveLength(1)
    expect(result.partitions).toHaveLength(1)
  })

  it('filters across places, bookmarks, and partitions by label or path', () => {
    const result = filterSidebarEntries({
      query: 'data',
      places: [
        { label: 'Home', path: '~' },
        { label: 'Recent', path: '' },
      ],
      bookmarks: [
        { label: 'Reference', path: '/tmp/reference' },
        { label: 'Dataset', path: '/srv/data/set-a' },
      ],
      partitions: [
        { label: 'Main Disk', path: '/mnt/main', removable: false },
        { label: 'Archive', path: '/mnt/data-archive', removable: true },
      ],
    })

    expect(result.places).toEqual([])
    expect(result.bookmarks).toEqual([{ label: 'Dataset', path: '/srv/data/set-a' }])
    expect(result.partitions).toEqual([{ label: 'Archive', path: '/mnt/data-archive', removable: true }])
  })
})

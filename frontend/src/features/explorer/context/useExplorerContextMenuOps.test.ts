import { describe, expect, it } from 'vitest'
import type { ContextAction } from './createContextMenus'
import { filterByCapabilities } from './useExplorerContextMenuOps'
import type { Entry } from '../model/types'

const entryWithCaps = (
  path: string,
  caps: NonNullable<Entry['capabilities']>,
): Entry => ({
  path,
  name: 'item',
  kind: 'file',
  iconId: 0,
  capabilities: caps,
})

const action = (id: string): ContextAction => ({ id, label: id })

describe('filterByCapabilities', () => {
  it('removes actions disabled by entry capabilities', () => {
    const actions: ContextAction[] = [
      action('copy'),
      action('cut'),
      action('rename'),
      action('move-trash'),
      action('delete-permanent'),
      action('properties'),
    ]
    const entry = entryWithCaps('rclone://remote/a.txt', {
      canList: true,
      canMkdir: true,
      canDelete: true,
      canRename: false,
      canMove: false,
      canCopy: true,
      canTrash: false,
      canUndo: false,
      canPermissions: false,
    })

    const filtered = filterByCapabilities(actions, [entry]).map((a) => a.id)

    expect(filtered).toEqual(['copy', 'delete-permanent', 'properties'])
  })

  it('keeps actions unchanged when capabilities are missing', () => {
    const actions: ContextAction[] = [action('copy'), action('rename')]
    const entry: Entry = { path: '/tmp/a', name: 'a', kind: 'file', iconId: 0 }

    expect(filterByCapabilities(actions, [entry]).map((a) => a.id)).toEqual(['copy', 'rename'])
  })

  it('recursively hides unsupported cloud additions while retaining allowed siblings', () => {
    const entry = entryWithCaps('rclone://remote/report.txt', {
      canList: true, canMkdir: true, canDelete: true, canRename: true,
      canMove: true, canCopy: true, canTrash: false, canUndo: false, canPermissions: false,
      canOpenWith: true, canArchive: false, canAdvancedRename: true, canExternalCopy: false,
    })
    const actions: ContextAction[] = [action('move-trash'), action('open-with'), action('rename-advanced'),
      action('cloud-export'), { id: 'archive', label: 'Archive', children: [action('compress'), action('extract')] },
      { id: 'other', label: 'Other', children: [action('copy'), action('compress')] }]
    const filtered = filterByCapabilities(actions, [entry])
    expect(filtered.map((item) => item.id)).toEqual(['open-with', 'rename-advanced', 'other'])
    expect(filtered[2].children?.map((item) => item.id)).toEqual(['copy'])
  })
})

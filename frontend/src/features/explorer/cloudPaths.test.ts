import { describe, expect, it } from 'vitest'
import { cloudBreadcrumbs, cloudDisplayPath, cloudLeafName, cloudParentPath, joinCloudPath } from './cloudPaths'
import { reconcileDirectorySnapshot } from './state/directorySnapshots'
import { clipboardState, setClipboardState, clearClipboardState } from './file-ops/clipboard.store'
import { get } from 'svelte/store'
import type { Entry } from './model/types'

const root = 'rclone://Google Disk'
const folder = `${root}//gdrive/parent~Documents`
const a = `${folder}/fileA~same.txt`
const b = `${folder}/fileB~same.txt`

describe('Google Drive object references', () => {
  it('keeps both rows and both clipboard selections across refresh order changes', () => {
    const entry = (path: string, size: number): Entry => ({ path, name: 'same.txt', kind: 'file', size, iconId: 0 })
    const first = reconcileDirectorySnapshot([], { current: folder, entries: [entry(a, 24), entry(b, 47)] }, false)
    expect(first.map(e => [e.path, e.size])).toEqual([[a, 24], [b, 47]])
    const refreshed = reconcileDirectorySnapshot(first, { current: folder, entries: [entry(b, 47), entry(a, 24)] }, true)
    expect(refreshed).toEqual(first)
    setClipboardState('copy', refreshed)
    expect([...get(clipboardState).paths]).toEqual([a, b])
    clearClipboardState()
  })
  it('renders human names while every breadcrumb retains the selected folder ID', () => {
    const path = `${folder}/nested~Budget+%C3%A5%7E%25`
    expect(cloudDisplayPath(path)).toBe(`${root}/Documents/Budget å~%`)
    expect(cloudLeafName(path)).toBe('Budget å~%')
    expect(cloudBreadcrumbs(path)).toEqual([
      { label: 'Google Disk', path: root }, { label: 'Documents', path: folder },
      { label: 'Budget å~%', path },
    ])
    expect(cloudParentPath(path)).toBe(folder)
    expect(cloudParentPath(folder)).toBe(root)
  })
  it('escapes destination names without turning them into IDs', () => {
    const path = joinCloudPath(folder, 'other~name 100%.txt')
    expect(path).toBe(`${folder}/~other%7Ename%20100%25.txt`)
    expect(cloudLeafName(path)).toBe('other~name 100%.txt')
    expect(cloudParentPath(path)).toBe(folder)
  })
  it('keeps the existing local and other-provider path behavior', () => {
    for (const path of ['/local/same.txt', 'rclone://Onedrive/Documents/same.txt']) {
      expect(cloudDisplayPath(path)).toBe(path)
      expect(cloudLeafName(path)).toBe('same.txt')
    }
    expect(joinCloudPath('rclone://Onedrive/Documents', 'a b.txt')).toBe('rclone://Onedrive/Documents/a b.txt')
  })
})

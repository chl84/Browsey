import { expect, it } from 'vitest'
import { applyDirectoryMetadata, reconcileDirectorySnapshot } from './directorySnapshots'
import type { Entry } from '../model/types'

const photo = (name: string): Entry => ({ name, path: `/camera/${name}`, kind: 'file', iconId: 14, size: 20, modified: '2026-01-01 12:00' })

it('keeps row references when a refresh changes nothing, including capability objects', () => {
  const first = reconcileDirectorySnapshot([], { current: '/camera', entries: [{ ...photo('a.jpg'), capabilities: { canList: false, canMkdir: false, canDelete: true, canRename: true, canMove: true, canCopy: true, canTrash: false, canUndo: false, canPermissions: false } }] }, false)
  const second = reconcileDirectorySnapshot(first, { current: '/camera', entries: [{ ...first[0], capabilities: { ...first[0].capabilities! } }] }, true)
  expect(second).toBe(first)
})

it('retains known revisions through a pending refresh but adopts genuinely changed metadata', () => {
  const first = reconcileDirectorySnapshot([], { current: '/camera', entries: [photo('a.jpg')] }, false)
  const placeholder = { ...photo('a.jpg'), size: null, modified: null, iconId: 0, kind: 'dir' as const }
  const pending = reconcileDirectorySnapshot(first, { current: '/camera', entries: [placeholder], pendingMetadataPaths: [placeholder.path] }, true)
  expect(pending[0]).toMatchObject({ size: 20, iconId: 14, kind: 'file', modified: '2026-01-01 12:00', metadataPending: true })
  const resolved = applyDirectoryMetadata(pending, new Map([[placeholder.path, { ...photo('a.jpg'), size: 30 }]]))
  expect(resolved[0]).toMatchObject({ size: 30, metadataPending: false })
  expect(applyDirectoryMetadata(resolved, new Map([[placeholder.path, { ...photo('a.jpg'), size: 30 }]]))).toBe(resolved)
})

it('does not interpret missing optional metadata as a placeholder without a backend marker', () => {
  const first = reconcileDirectorySnapshot([], { current: '/camera', entries: [photo('a.jpg')] }, false)
  const next = reconcileDirectorySnapshot(first, { current: '/camera', entries: [{ ...photo('a.jpg'), size: null, modified: null, readDenied: true }] }, true)
  expect(next[0]).toMatchObject({ size: null, modified: null, readDenied: true, metadataPending: false })
})

it('never restores deleted files or applies early metadata to an authoritative complete entry', () => {
  const first = reconcileDirectorySnapshot([], { current: '/camera', entries: [photo('a.jpg'), photo('b.jpg')] }, false)
  const early = new Map([[first[0].path, { ...photo('a.jpg'), size: 999 }]])
  const next = reconcileDirectorySnapshot(first, { current: '/camera', entries: [photo('a.jpg')] }, true, early)
  expect(next.map(entry => entry.name)).toEqual(['a.jpg'])
  expect(next[0].size).toBe(20)
})

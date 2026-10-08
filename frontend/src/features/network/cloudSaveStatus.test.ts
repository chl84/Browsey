import { expect, it } from 'vitest'
import { cloudSaveSummary, mergeCloudSaveStatus } from './cloudSaveStatus'
import type { CloudWritebackStatus } from './cloud.service'
const row = (id: string, status: CloudWritebackStatus['status'], sequence: number): CloudWritebackStatus => ({ id, name: 'same.txt', sourcePath: `rclone://test/${id}`, status, message: null, sequence, bytes: 8, total: 8, saveCompleted: false })
it('retains both working copies with identical filenames', () => {
  const rows = mergeCloudSaveStatus([row('first', 'saved', 1)], row('second', 'uploading', 2))
  expect(rows).toHaveLength(2)
  expect(cloudSaveSummary(rows)).toBe('Saving 1…')
})
it('does not treat complete byte progress as confirmed cloud saving', () => {
  expect(cloudSaveSummary([row('first', 'uploading', 1)])).toBe('Saving 1…')
  expect(cloudSaveSummary([row('first', 'saved', 2)])).toBe('Saved')
})
it('a late initial snapshot cannot hide a live conflict', () => {
  const rows = [row('first', 'conflict', 3)]
  expect(mergeCloudSaveStatus(rows, row('first', 'saved', 1))).toBe(rows)
  expect(cloudSaveSummary(rows)).toBe('1 need attention')
  expect(mergeCloudSaveStatus(rows, row('first', 'saved', 4))[0].status).toBe('saved')
})

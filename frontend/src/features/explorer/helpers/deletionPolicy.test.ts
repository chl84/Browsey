import { expect, it } from 'vitest'
import { mustUsePermanentDelete } from './deletionPolicy'
import type { Entry } from '../model/types'

const remote: Entry = { name: 'large.bin', path: '/run/user/1000/gvfs/sftp:host=server/large.bin', kind: 'file', iconId: 0, network: true }
it('tries backend trash for ordinary Delete on Linux network/phone files', () => {
  expect(mustUsePermanentDelete([remote], false, false, false)).toBe(false)
})
it('retains Shift+Delete and the Windows network flow', () => {
  expect(mustUsePermanentDelete([remote], true, false, false)).toBe(true)
  expect(mustUsePermanentDelete([remote], false, false, true)).toBe(true)
})
it('retains permanent deletion for providers without supported trash', () => {
  expect(mustUsePermanentDelete([{ ...remote, path: 'rclone://remote/file' }], false, false, false)).toBe(true)
})

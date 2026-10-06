import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { noLinks } from './scope.mjs'
import { privateStat } from './privacy.mjs'

const backends = ['filesystem-directory', 'gio', 'owned-stream', 'owned-stream-receipt',
  'filesystem-rename', 'copy-verify-delete', 'rclone-cli', 'rclone-provider-upload', 'rclone-provider-download']
export function parseRouting(text, operation) {
  assert.ok(['copy', 'move'].includes(operation))
  const events = []
  for (const line of text.split('\n').filter(line => line.includes('transfer dispatch'))) {
    const field = name => line.match(new RegExp(`\\b${name}="([a-z-]+)"`))?.[1]
    const op = field('op'), backend = field('backend'), kind = field('kind'), staging = field('staging')
    assert.ok(['copy', 'move'].includes(op) && backends.includes(backend)
      && ['file', 'directory', 'entry'].includes(kind) && staging === 'direct', 'Unknown transfer dispatch receipt')
    if (op === operation) events.push({ backend, kind, staging })
  }
  assert.ok(events.length >= 3 && events.length <= 64, 'Missing or excessive mixed-batch dispatch receipts')
  assert.ok(events.some(e => ['file', 'entry'].includes(e.kind)) && events.some(e => ['directory', 'entry'].includes(e.kind)),
    'Routing must cover files and folders')
  return { operation, staging: 'direct-destination', events,
    limitation: 'Dispatch receipts plus independent trees; provider-internal temporary files are not certified' }
}

export function routeEvidence(raw) {
  const read = async () => {
    await noLinks(raw, fs)
    const handle = await fs.open(raw, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW)
    try {
      const stat = await handle.stat(); privateStat(stat)
      assert.ok(stat.size <= 1024 * 1024, 'Routing log exceeded bounded read budget')
      return { inode: String(stat.ino), bytes: await handle.readFile() }
    } finally { await handle.close() }
  }
  return { mark: async () => { const entry = await read(); return { inode: entry.inode, size: entry.bytes.length } },
    read: async (start, operation) => {
      const entry = await read()
      assert.ok(entry.inode === start.inode && entry.bytes.length >= start.size, 'Routing log rotated during operation')
      return parseRouting(entry.bytes.subarray(start.size).toString('utf8'), operation)
    } }
}

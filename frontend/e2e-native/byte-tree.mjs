import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'

export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')
// Bounded independent metadata and binary readback. No UTF-8 decoding, mutation
// replay, unexpected-directory descent, or unawaited verification subprocesses.
export async function verifyByteTree(fixture, base, expected, { maxEntries = 128, maxDepth = 8 } = {}) {
  assert.ok(Number.isSafeInteger(maxEntries) && maxEntries > 0 && maxEntries <= 256)
  assert.ok(Number.isSafeInteger(maxDepth) && maxDepth > 0 && maxDepth <= 16)
  assert.ok(expected.size <= maxEntries && [...expected.values()].every(value => value === null || Buffer.isBuffer(value)))
  const reads = async actions => {
    const results = await Promise.allSettled(actions.map(action => Promise.resolve().then(action)))
    for (const result of results) if (result.status === 'rejected') throw result.reason
    return results.map(result => result.value)
  }
  const enumerate = async () => {
    const actual = new Map(), files = [], pending = [{ path: base, depth: 0 }]
    while (pending.length) {
      const group = pending.splice(0, 2)
      assert.ok(group.every(directory => directory.depth <= maxDepth), 'Explicit independent tree depth budget')
      const snapshots = await reads(group.map(directory => () => fixture.snapshot(directory.path, { maxChildren: maxEntries })))
      for (const [index, entries] of snapshots.entries()) for (const entry of entries) {
        assert.ok(entry.path.startsWith(`${base}/`) && ['file', 'dir'].includes(entry.kind))
        const name = entry.path.slice(base.length + 1)
        assert.ok(!actual.has(name) && actual.size < maxEntries, 'Independent tree entry budget or duplicate entry')
        actual.set(name, entry.kind)
        if (entry.kind === 'file') files.push({ ...entry, relative: name })
        else if (expected.has(name) && expected.get(name) === null) pending.push({ path: entry.path, depth: group[index].depth + 1 })
      }
    }
    return { actual, files }
  }
  const expectedShape = [...expected].map(([name, value]) => [name, value === null ? 'dir' : 'file']).sort()
  let actual, files
  for (let attempt = 0; ; attempt++) {
    ({ actual, files } = await enumerate())
    const shape = [...actual].sort()
    if (JSON.stringify(shape) === JSON.stringify(expectedShape)) break
    if (!base.startsWith('rclone://') || attempt === 3) assert.deepEqual(shape, expectedShape, 'Exact generated membership before byte reads')
    await new Promise(resolve => setTimeout(resolve, 1000))
  }
  const digests = []
  for (let offset = 0; offset < files.length; offset += 2) {
    const group = files.slice(offset, offset + 2)
    const contents = await reads(group.map(file => () => fixture.readBytes(file.path)))
    for (const [index, entry] of group.entries()) {
      const bytes = contents[index], wanted = expected.get(entry.relative)
      assert.ok(Buffer.isBuffer(bytes), 'Binary verification must not decode filenames or content bytes')
      assert.equal(entry.size, wanted.length, 'Independent metadata size')
      assert.deepEqual(bytes, wanted, 'Exact independently read file bytes')
      const digest = sha256(bytes); assert.equal(digest, sha256(wanted))
      digests.push({ name: entry.relative, bytes: bytes.length, sha256: digest })
    }
  }
  return { entries: actual.size, digests: digests.sort((a, b) => a.name.localeCompare(b.name)) }
}

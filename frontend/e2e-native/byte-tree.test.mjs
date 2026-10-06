import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { Fixtures } from './fixtures.mjs'
import { verifyByteTree } from './byte-tree.mjs'

test('byte tree detects same-size binary corruption and refuses unbounded traversal', async t => {
  const base = await fs.mkdtemp(path.join(os.tmpdir(), 'browsey-byte-tree-'))
  t.after(() => fs.rm(base, { recursive: true, force: true }))
  const fixture = new Fixtures({ targets: [{ kind: 'local', files: base }] }, {})
  const bytes = Buffer.from([255, 0, 128, 254]), expected = new Map([['data.bin', bytes]])
  await fixture.write(`${base}/data.bin`, bytes)
  assert.equal((await verifyByteTree(fixture, base, expected)).digests[0].bytes, 4)
  await fs.writeFile(`${base}/data.bin`, Buffer.from([254, 0, 128, 255]))
  await assert.rejects(verifyByteTree(fixture, base, expected), /Exact independently read file bytes/)
  for (const options of [{ maxEntries: 257 }, { maxEntries: Infinity }, { maxDepth: 17 }, { maxDepth: 0 }]) {
    await assert.rejects(verifyByteTree(fixture, base, expected, options))
  }
})

test('byte verification refuses unexpected directories without descending into them', async () => {
  const read = []
  const fixture = { snapshot: async base => {
    read.push(base)
    assert.equal(base, '/owned', 'Unexpected directory must never be traversed')
    return [{ path: '/owned/unexpected', kind: 'dir' }]
  } }
  await assert.rejects(verifyByteTree(fixture, '/owned', new Map()), /Exact generated membership/)
  assert.deepEqual(read, ['/owned'])
})

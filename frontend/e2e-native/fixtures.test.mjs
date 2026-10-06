import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { Fixtures } from './fixtures.mjs'

test('shallow fixture metadata is independent and rejects symlinks and oversized generated listings', async () => {
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'browsey-listing-policy-'))
  const files = path.join(temp, 'files')
  try {
    await fs.mkdir(files)
    const fixture = new Fixtures({ targets: [{ kind: 'local', files }] }, {})
    await fixture.write(`${files}/Alpha.TXT`, 'generated')
    await fixture.mkdir(`${files}/empty`)
    const snapshot = await fixture.snapshot(files)
    assert.equal(snapshot.find(entry => entry.name === 'Alpha.TXT').size, 9)
    assert.equal(snapshot.find(entry => entry.name === 'empty').kind, 'dir')
    assert.ok(snapshot.every(entry => Number.isSafeInteger(entry.modifiedMinute)))
    await fs.symlink(`${files}/Alpha.TXT`, `${files}/link`)
    await assert.rejects(fixture.snapshot(files), /symlink/)
    await fs.unlink(`${files}/link`)
    for (let index = 0; index < 31; index++) await fixture.write(`${files}/generated-${index}.txt`, 'x')
    await assert.rejects(fixture.snapshot(files), /32 generated children/)
    assert.equal((await fixture.snapshot(files, { maxChildren: 256 })).length, 33)
    for (const maxChildren of [0, 257, Infinity, 1.5]) await assert.rejects(fixture.snapshot(files, { maxChildren }), /bound/)
  } finally { await fs.rm(temp, { recursive: true, force: true }) }
})

test('binary fixture readback preserves invalid UTF-8 bytes and enforces the 64 KiB write budget', async t => {
  const files = await fs.mkdtemp(path.join(os.tmpdir(), 'browsey-binary-fixture-'))
  t.after(() => fs.rm(files, { recursive: true, force: true }))
  const fixture = new Fixtures({ targets: [{ kind: 'local', files }] }, {})
  const bytes = Buffer.from([0, 255, 254, 128, 195, 40])
  await fixture.write(`${files}/generated.bin`, bytes)
  assert.deepEqual(await fixture.readBytes(`${files}/generated.bin`), bytes)
  await assert.rejects(fixture.write(`${files}/oversized.bin`, Buffer.alloc(65537)), /fixtures are small/)
  assert.equal(await fixture.exists(`${files}/oversized.bin`), false)
})

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

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { editingManifest, editing, verifyTree, propertiesSizeMatches } from './editing.mjs'
import { ownedRestart } from './restart.mjs'
import { makePlan, validateConfig, kinds, ownedPath } from './scope.mjs'
import { createReport, recordCase, finishReport } from './report.mjs'
const plan = makePlan(validateConfig({ schema: 1, matrix: 'local-hub', targets: Object.fromEntries(kinds.map(kind => [kind,
  kind === 'cloud' ? 'rclone://test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])), rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '23456789-1234-4234-9234-123456789abc')

test('Properties size comparison tolerates DOM whitespace but rejects incorrect totals, counts and unknown values', () => {
  assert.ok(propertiesSizeMatches('42 B(1item)', '42 B (1 item)'))
  assert.ok(propertiesSizeMatches(' 42 B \n ( 2 items ) ', '42 B (2 items)'))
  for (const actual of [undefined, '0 B (1 item)', '42 B (2 items)', '42 kB (1 item)', '—', '42 B (1 item) extra']) {
    assert.equal(propertiesSizeMatches(actual, '42 B (1 item)'), false)
  }
})

test('combined editing declares all five rename/transfer/properties scopes and local history only', () => {
  const manifest = editingManifest(plan), report = createReport(plan, kinds, manifest)
  assert.equal(manifest.length, 16)
  for (const kind of kinds) for (const prefix of ['fileops', 'rename-edge', 'properties']) assert.ok(manifest.some(item => item.id === `${prefix}-${kind}`))
  assert.deepEqual(manifest.find(item => item.id === 'history-local').providers, ['local'])
  assert.ok(report.cases.every(item => Object.values(item.requirements).every(caps => caps.length)))
  assert.equal(editingManifest(plan, 'history').length, 1)
  assert.equal(editingManifest(plan, 'rename').length, 5)
})

test('tree verification catches missing/extra paths, cross-kind changes and modified nested bytes', async () => {
  const expected = new Map([['tree', null], ['tree/nested.txt', 'original'], ['unrelated.txt', 'preserve']])
  for (const variant of [new Map(expected), new Map([...expected].slice(1)), new Map([...expected, ['extra', 'bad']]),
    new Map([...expected, ['tree/nested.txt', 'truncated']]), new Map([...expected, ['tree', 'file now']])]) {
    const fixture = { snapshot: async path => [...variant].filter(([key]) => `/owned/${key}`.slice(0, `/owned/${key}`.lastIndexOf('/')) === path)
      .map(([key, value]) => ({ path: `/owned/${key}`, kind: value === null ? 'dir' : 'file' })), read: async path => variant.get(path.slice('/owned/'.length)) }
    if (variant === expected || JSON.stringify([...variant]) === JSON.stringify([...expected])) await verifyTree(fixture, '/owned', expected)
    else await assert.rejects(verifyTree(fixture, '/owned', expected), /Exact generated paths/)
  }
})

test('tree reads use at most two children and await a started sibling after failure', async () => {
  const expected = new Map([['first', 'one'], ['second', 'two'], ['third', 'three']])
  for (const fail of [false, true]) {
    let active = 0, maximum = 0, completed = 0, calls = 0
    const fixture = { snapshot: async () => [...expected.keys()].map(name => ({ path: `/owned/${name}`, kind: 'file' })),
      read: async path => {
        calls++; maximum = Math.max(maximum, ++active)
        try {
          await new Promise(resolve => setTimeout(resolve, path.endsWith('second') ? 10 : 1))
          if (fail && path.endsWith('first')) throw Error('read failed')
          completed++; return expected.get(path.slice('/owned/'.length))
        } finally { active-- }
      } }
    if (fail) { await assert.rejects(verifyTree(fixture, '/owned', expected), /read failed/); assert.equal(calls, 2); assert.equal(completed, 1) }
    else await verifyTree(fixture, '/owned', expected)
    assert.equal(active, 0); assert.equal(maximum, 2)
  }
})

test('tree verification rejects a prefix sibling before any content read', async () => {
  let reads = 0
  await assert.rejects(verifyTree({ snapshot: async () => [{ path: '/owned-other/file', kind: 'file' }],
    read: async () => { reads++; return 'bytes' } }, '/owned', new Map([['file', 'bytes']])), /below this owned case/)
  assert.equal(reads, 0)
})

test('cloud readiness ignores a transient deleted directory before reading bytes and never retries damaged content', async () => {
  const base = 'rclone://test/owned', expected = new Map([['file', 'preserved']])
  for (const damaged of [false, true]) {
    let snapshots = 0, reads = 0
    const fixture = { snapshot: async path => {
      assert.equal(path, base, 'A stale unexpected directory must not be traversed')
      snapshots++
      return [{ path: `${base}/file`, kind: 'file' }, ...(snapshots === 1 ? [{ path: `${base}/deleted`, kind: 'dir' }] : [])]
    }, read: async path => {
      assert.equal(snapshots, 2); assert.equal(path, `${base}/file`); reads++
      return damaged ? 'damaged' : 'preserved'
    } }
    if (damaged) await assert.rejects(verifyTree(fixture, base, expected), /Exact generated paths/)
    else await verifyTree(fixture, base, expected)
    assert.equal(snapshots, 2); assert.equal(reads, 1)
  }
})

test('cloud readiness bounds stale metadata and never retries a failed listing', async () => {
  const base = 'rclone://test/owned'
  for (const ioFailure of [false, true]) {
    let snapshots = 0, reads = 0
    const fixture = { snapshot: async path => {
      assert.equal(path, base); snapshots++
      if (ioFailure) throw Error('listing transport failed')
      return [{ path: `${base}/file`, kind: 'file' }, { path: `${base}/stale-directory`, kind: 'dir' }]
    }, read: async () => { reads++; return 'preserved' } }
    await assert.rejects(verifyTree(fixture, base, new Map([['file', 'preserved']])),
      ioFailure ? /listing transport failed/ : /Exact generated paths/)
    assert.equal(snapshots, ioFailure ? 1 : 4); assert.equal(reads, 0)
  }
})

test('acknowledged no-selection input cannot pass after unrelated bytes change; later providers remain unrun', async () => {
  const roots = plan.targets.map(t => t.files), items = new Map(), report = createReport(plan, kinds, editingManifest(plan, 'fileops'))
  const fixture = { mkdir: async path => { ownedPath(roots, path); items.set(path, null) },
    write: async (path, bytes) => { ownedPath(roots, path); items.set(path, bytes) },
    read: async path => items.get(path), snapshot: async path => [...items].filter(([key]) => key.slice(0, key.lastIndexOf('/')) === path)
      .map(([path, value]) => ({ path, kind: value === null ? 'dir' : 'file' })) }
  let attempts = 0
  const ui = { navigate: async () => {}, setView: async () => {}, emptySpace: async () => {}, chord: async () => { attempts++ }, idle: async () => {}, expectedToast: async () => {},
    selection: async () => { const path = [...items.keys()].find(x => x.endsWith('/unrelated.txt')); items.set(path, 'damaged') } }
  await assert.rejects(editing(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action), 'fileops'), /Exact generated paths/)
  finishReport(report, new Error('Observed independent result mismatch'))
  assert.equal(report.status, 'FAIL'); assert.equal(report.cases[0].failureKind, 'RESULT_MISMATCH')
  assert.equal(report.cases[0].parts[0].ui, 'ACKNOWLEDGED'); assert.equal(attempts, 4)
  assert.ok(report.cases.slice(1).every(item => item.status === 'NOT_RUN'))
})

for (const fail of ['teardown', 'start', null]) test(`owned restart is one attempt, preserves closure evidence and refuses failed teardown (${fail})`, async () => {
  const calls = [], restarts = []
  const restart = ownedRestart({ restarts, persist: async () => { calls.push('persist') },
    stop: async () => { calls.push('stop'); return { candidate: { pid: 123 }, teardown: { status: fail === 'teardown' ? 'BLOCKED' : 'PASS' } } },
    start: async () => { calls.push('start'); if (fail === 'start') throw Error('start failed'); return { candidate: { pid: 456 } } } })
  if (fail) await assert.rejects(restart(), fail === 'start' ? /start failed/ : /unconfirmed owned teardown/)
  else await restart()
  assert.equal(restarts.length, 1); assert.equal(restarts[0].status, fail ? 'BLOCKED' : 'PASS')
  assert.equal(calls.filter(x => x === 'start').length, fail === 'teardown' ? 0 : 1)
  await assert.rejects(restart(), /Only one explicit/)
  assert.equal(calls.filter(x => x === 'stop').length, 1)
})

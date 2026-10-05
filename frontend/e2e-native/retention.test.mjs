import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { createServer } from 'node:net'
import { validateConfig, makePlan } from './scope.mjs'
import { RetentionStore } from './retention.mjs'
import { writePrivate, privateJson, inspectTree, processStamp, retentionPolicy } from './privacy.mjs'

const now = Date.now()
const old = new Date(now - 8 * 86400_000).toISOString()
const dead = { pid: 2147483647, start: '1' }
async function fixture(t, { policy, providers = ['local'] } = {}) {
  const temporary = await fs.mkdtemp('/tmp/n')
  t.after(() => fs.rm(temporary, { recursive: true }))
  const root = `${temporary}/ai_agent_testfolder`
  await fs.mkdir(root, { mode: 0o700 })
  const config = validateConfig({ schema: 1, targets: { local: root } })
  const plan = makePlan(config, randomUUID())
  const store = new RetentionStore(`${temporary}/registry`, { policy })
  const record = await store.reserve(plan)
  const run = plan.targets[0].run
  await fs.mkdir(run, { mode: 0o700 })
  await fs.mkdir(`${run}/files`, { mode: 0o700 })
  await writePrivate(`${run}/files/generated.txt`, 'fixture recovery data', { exclusive: true })
  const owner = { schema: 2, runId: plan.runId, nonce: record.nonce, rootHash: record.rootHash,
    runner: dead, createdAt: old, selectedProviders: providers, policy: 1 }
  const report = { schema: 3, runId: plan.runId, status: 'PASS', build: { feature: 'native-test' }, finished: old,
    cases: [{ id: 'synthetic', status: 'PASS' }], driverIdentity: { pid: dead.pid, startTime: dead.start,
      nativePid: dead.pid, nativeStartTime: dead.start }, identity: { pid: dead.pid, startTime: dead.start },
    teardownDetails: { status: 'PASS', steps: ['session-close', 'driver-exit', 'native-driver-exit', 'candidate-exit']
      .map(stage => ({ stage, status: 'PASS' })) } }
  await writePrivate(`${run}/owner.json`, JSON.stringify(owner), { exclusive: true })
  await writePrivate(`${run}/report.json`, JSON.stringify(report), { exclusive: true })
  await store.complete(plan.runId, await inspectTree(run), 'PASS')
  await writePrivate(`${root}/unrelated.txt`, 'unrelated approved-root sentinel', { exclusive: true })
  const saveOwner = () => writePrivate(`${run}/owner.json`, JSON.stringify(owner))
  const saveReport = () => writePrivate(`${run}/report.json`, JSON.stringify(report))
  return { store, plan, config, root, run, owner, report, record, temporary, saveOwner, saveReport }
}

test('expanded finite count preserves registered recovery while still stopping at the new limit', async t => {
  const f = await fixture(t)
  const registryPath = `${f.temporary}/registry/registry.json`
  const registry = await privateJson(registryPath)
  const original = { ...registry.runs[0] }
  while (registry.runs.length < retentionPolicy.maxRuns - 1) registry.runs.push({ ...original, runId: randomUUID(), bytes: 0 })
  await writePrivate(registryPath, JSON.stringify(registry))
  await f.store.reserve(makePlan(f.config, randomUUID()))
  await assert.rejects(f.store.reserve(makePlan(f.config, randomUUID())), /count budget/)
  const after = await privateJson(registryPath)
  assert.deepEqual(after.runs.slice(0, registry.runs.length), registry.runs)
  assert.equal(await fs.readFile(`${f.run}/files/generated.txt`, 'utf8'), 'fixture recovery data')
})

test('reviewed cleanup removes only one eligible synthetic run, preserving its approved root and unrelated contents', async t => {
  const f = await fixture(t)
  const review = await f.store.plan(f.config, f.plan.runId, { now })
  assert.equal(review.eligible, true)
  assert.equal((await f.store.audit(f.config, f.plan.runId)).private, true)
  const result = await f.store.cleanup(f.config, f.plan.runId, review.planSha256, { now })
  assert.equal(result.status, 'PASS')
  await assert.rejects(fs.lstat(f.run), error => error.code === 'ENOENT')
  assert.ok((await fs.stat(f.root)).isDirectory())
  assert.equal(await fs.readFile(`${f.root}/unrelated.txt`, 'utf8'), 'unrelated approved-root sentinel')
  assert.deepEqual((await privateJson(`${f.temporary}/registry/registry.json`)).runs, [])
})

test('failed, blocked, young, future-dated and incompletely torn-down runs preserve recovery data', async t => {
  for (const changed of [{ status: 'FAIL' }, { status: 'BLOCKED' }, { finished: new Date(now).toISOString() },
    { finished: new Date(now + 86400_000).toISOString() }, { teardownDetails: { status: 'BLOCKED' } }]) {
    const f = await fixture(t)
    Object.assign(f.report, changed); await f.saveReport()
    await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }))
    assert.equal(await fs.readFile(`${f.run}/files/generated.txt`, 'utf8'), 'fixture recovery data')
  }
})

test('active marker, live recorded process and multi-provider recovery each prevent cleanup', async t => {
  const f = await fixture(t)
  await writePrivate(`${f.run}/active.json`, '{}', { exclusive: true })
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /Active run/)
  await fs.unlink(`${f.run}/active.json`)
  f.owner.runner = { pid: process.pid, start: (await processStamp(process.pid)).start }; await f.saveOwner()
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /still alive/)
  f.owner.runner = dead; f.owner.selectedProviders = ['local', 'mobile']; await f.saveOwner()
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /Multi-provider recovery/)
  assert.equal(await fs.readFile(`${f.run}/files/generated.txt`, 'utf8'), 'fixture recovery data')
})

test('unregistered legacy runs, mismatched nonce/root and traversal UUIDs cannot be adopted', async t => {
  const f = await fixture(t)
  await assert.rejects(f.store.plan(f.config, '../ai_agent_testfolder', { now }), /canonical run UUID/)
  await assert.rejects(f.store.plan(f.config, randomUUID(), { now }), /unregistered/)
  f.owner.schema = 1; await f.saveOwner()
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /Legacy ownership/)
  f.owner.schema = 2; f.owner.nonce = randomUUID(); await f.saveOwner()
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /ownership differs/)
  f.owner.nonce = f.record.nonce; f.owner.rootHash = '0'.repeat(64); await f.saveOwner()
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /Expected values/)
})

test('changed files, added entries, hardlinks and nested symlinks invalidate the plan before deletion', async t => {
  for (const change of ['modify', 'add', 'hardlink', 'symlink']) {
    const f = await fixture(t)
    const review = await f.store.plan(f.config, f.plan.runId, { now })
    if (change === 'modify') await writePrivate(`${f.run}/files/generated.txt`, 'new generated contents')
    if (change === 'add') await writePrivate(`${f.run}/files/new`, 'new', { exclusive: true })
    if (change === 'hardlink') await fs.link(`${f.root}/unrelated.txt`, `${f.run}/files/alias`)
    if (change === 'symlink') await fs.symlink(`${f.root}/unrelated.txt`, `${f.run}/files/alias`)
    await assert.rejects(f.store.cleanup(f.config, f.plan.runId, review.planSha256, { now }))
    assert.ok((await fs.stat(`${f.run}/report.json`)).isFile())
    assert.equal(await fs.readFile(`${f.root}/unrelated.txt`, 'utf8'), 'unrelated approved-root sentinel')
  }
})

test('identity replacement during cleanup is blocked, preserves control metadata and marks uncertainty without retry', async t => {
  const f = await fixture(t)
  const review = await f.store.plan(f.config, f.plan.runId, { now })
  await assert.rejects(f.store.cleanup(f.config, f.plan.runId, review.planSha256, { now, beforeDelete: async entry => {
    if (entry.path === 'files/generated.txt') {
      await fs.unlink(`${f.run}/files/generated.txt`)
      await fs.symlink(`${f.root}/unrelated.txt`, `${f.run}/files/generated.txt`)
    }
  } }), error => error.failureKind === 'CLEANUP_UNCERTAIN' && error.removed === 0)
  assert.equal((await privateJson(`${f.temporary}/registry/registry.json`)).runs[0].state, 'UNCERTAIN')
  assert.ok((await fs.stat(`${f.run}/owner.json`)).isFile())
  assert.ok((await fs.stat(`${f.run}/report.json`)).isFile())
  assert.equal(await fs.readFile(`${f.root}/unrelated.txt`, 'utf8'), 'unrelated approved-root sentinel')
  await assert.rejects(f.store.reserve(makePlan(f.config, randomUUID())), /explicit review/)
})

test('count/byte reservations and unresolved audit uncertainty block new runs without automatic expiry deletion', async t => {
  const f = await fixture(t, { policy: { schema: 1, successDays: 7, maxRunBytes: 4096, maxTotalBytes: 8192,
    maxRuns: 2, maxEntries: 100, maxDepth: 10, auditMs: 1000 } })
  const second = await f.store.reserve(makePlan(f.config, randomUUID()))
  await assert.rejects(f.store.reserve(makePlan(f.config, randomUUID())), /count budget/)
  await f.store.complete(second.runId, null, 'BLOCKED')
  await assert.rejects(f.store.reserve(makePlan(f.config, randomUUID())), /explicit review/)
  assert.equal(await fs.readFile(`${f.run}/files/generated.txt`, 'utf8'), 'fixture recovery data')
  const g = await fixture(t, { policy: { schema: 1, successDays: 7, maxRunBytes: 4096, maxTotalBytes: 4096,
    maxRuns: 20, maxEntries: 100, maxDepth: 10, auditMs: 1000 } })
  await assert.rejects(g.store.reserve(makePlan(g.config, randomUUID())), /byte budget/)
})

test('a runtime socket prevents cleanup even when recorded app and driver processes are gone', async t => {
  const f = await fixture(t)
  const server = createServer()
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(`${f.run}/socket`, resolve) })
  t.after(() => new Promise(resolve => server.close(resolve)))
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /sidecar process ownership/)
  assert.equal(await fs.readFile(`${f.run}/files/generated.txt`, 'utf8'), 'fixture recovery data')
})

test('existing registry lock and non-private registry deny writes rather than stealing ownership', async t => {
  const f = await fixture(t)
  await writePrivate(`${f.temporary}/registry/registry.lock`, 'foreign lock sentinel', { exclusive: true })
  await assert.rejects(f.store.reserve(makePlan(f.config, randomUUID())), error => error.code === 'EEXIST')
  assert.equal(await fs.readFile(`${f.temporary}/registry/registry.lock`, 'utf8'), 'foreign lock sentinel')
  await fs.unlink(`${f.temporary}/registry/registry.lock`)
  await fs.chmod(`${f.temporary}/registry/registry.json`, 0o644)
  await assert.rejects(f.store.reserve(makePlan(f.config, randomUUID())), /mode 700\/600/)
})

test('explicit uncertain-run review requires stopped processes and valid privacy, retaining failure and recovery data', async t => {
  const f = await fixture(t)
  f.report.status = 'BLOCKED'; await f.saveReport()
  await f.store.complete(f.plan.runId, null, 'BLOCKED')
  await writePrivate(`${f.run}/active.json`, JSON.stringify({ runId: f.plan.runId, nonce: f.record.nonce }), { exclusive: true })
  f.owner.runner = { pid: process.pid, start: (await processStamp(process.pid)).start }; await f.saveOwner()
  await assert.rejects(f.store.review(f.config, f.plan.runId), /still alive/)
  f.owner.runner = dead; await f.saveOwner()
  await fs.chmod(`${f.run}/files/generated.txt`, 0o644)
  await assert.rejects(f.store.review(f.config, f.plan.runId), /mode 700\/600/)
  await fs.chmod(`${f.run}/files/generated.txt`, 0o600)
  const reviewed = await f.store.review(f.config, f.plan.runId)
  assert.equal(reviewed.state, 'RETAINED')
  assert.equal(reviewed.status, 'BLOCKED')
  assert.equal((await privateJson(`${f.run}/report.json`)).status, 'BLOCKED')
  assert.equal(await fs.readFile(`${f.run}/files/generated.txt`, 'utf8'), 'fixture recovery data')
  await assert.rejects(fs.stat(`${f.run}/active.json`), error => error.code === 'ENOENT')
  await assert.rejects(f.store.plan(f.config, f.plan.runId, { now }), /Failed\/blocked/)
  await f.store.reserve(makePlan(f.config, randomUUID()))
})

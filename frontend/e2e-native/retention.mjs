import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { randomUUID, createHash } from 'node:crypto'
import { child, noLinks, makePlan } from './scope.mjs'
import { retentionPolicy, rootHash, privateStat, privateJson, writePrivate, inspectTree,
  processGone, verifyEntry } from './privacy.mjs'

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
const digest = value => createHash('sha256').update(JSON.stringify(value)).digest('hex')

export class RetentionStore {
  constructor(directory, { policy = retentionPolicy } = {}) { this.directory = directory; this.policy = policy }

  async #prepare() {
    await noLinks(this.directory, fs)
    try { await fs.mkdir(this.directory, { mode: 0o700 }) } catch (error) { if (error.code !== 'EEXIST') throw error }
    privateStat(await fs.lstat(this.directory), true)
  }

  async #load() {
    let registry
    try { registry = await privateJson(child(this.directory, 'registry.json')) }
    catch (error) { if (error.code !== 'ENOENT') throw error; registry = { schema: 1, runs: [] } }
    assert.equal(registry.schema, 1)
    assert.ok(Array.isArray(registry.runs) && registry.runs.length <= this.policy.maxRuns)
    const ids = new Set()
    for (const run of registry.runs) {
      assert.match(run.runId, uuid)
      assert.match(run.nonce, uuid)
      assert.match(run.rootHash, /^[a-f0-9]{64}$/)
      assert.ok(!ids.has(run.runId) && Number.isSafeInteger(run.bytes) && run.bytes >= 0)
      assert.ok(['RESERVED', 'RETAINED', 'UNCERTAIN'].includes(run.state))
      ids.add(run.runId)
    }
    return registry
  }

  async #save(registry) {
    const temporary = child(this.directory, `.registry-${randomUUID()}`)
    await writePrivate(temporary, JSON.stringify(registry, null, 2), { exclusive: true })
    try { await fs.rename(temporary, child(this.directory, 'registry.json')) }
    catch (error) { await fs.unlink(temporary); throw error }
  }

  async #locked(action) {
    await this.#prepare()
    const raw = child(this.directory, 'registry.lock')
    const lock = await fs.open(raw, 'wx', 0o600)
    const identity = await lock.stat({ bigint: true })
    try { return await action() }
    finally {
      await lock.close()
      const current = await fs.lstat(raw, { bigint: true })
      assert.ok(current.dev === identity.dev && current.ino === identity.ino, 'Registry lock identity changed; refuse cleanup')
      await fs.unlink(raw)
    }
  }

  async reserve(plan) {
    return this.#locked(async () => {
      const registry = await this.#load()
      assert.ok(!registry.runs.some(run => run.state === 'UNCERTAIN'), 'Uncertain retained data requires explicit review before another run')
      assert.ok(!registry.runs.some(run => run.runId === plan.runId), 'Cannot reuse a retained run')
      assert.ok(registry.runs.length < this.policy.maxRuns, 'Native retained-run count budget reached; no automatic deletion')
      assert.ok(registry.runs.reduce((sum, run) => sum + run.bytes, 0) + this.policy.maxRunBytes <= this.policy.maxTotalBytes,
        'Native retained-byte budget reached; no automatic deletion')
      const record = { runId: plan.runId, nonce: randomUUID(), rootHash: rootHash(plan.targets.find(t => t.kind === 'local').path),
        bytes: this.policy.maxRunBytes, state: 'RESERVED', createdAt: new Date().toISOString() }
      registry.runs.push(record)
      await this.#save(registry)
      return record
    })
  }

  async complete(runId, audit, status) {
    return this.#locked(async () => {
      const registry = await this.#load()
      const record = registry.runs.find(run => run.runId === runId)
      assert.ok(record, 'Run must be reserved before recording retained data')
      assert.ok(!audit || (Number.isSafeInteger(audit.bytes) && audit.bytes >= 0 && audit.bytes <= this.policy.maxRunBytes))
      Object.assign(record, { bytes: audit?.bytes ?? this.policy.maxRunBytes, state: audit ? 'RETAINED' : 'UNCERTAIN', status })
      await this.#save(registry)
    })
  }

  async #owned(config, runId) {
    assert.match(runId, uuid, 'Specify exactly one canonical run UUID')
    const registry = await this.#load()
    const record = registry.runs.find(run => run.runId === runId)
    assert.ok(record, 'Cleanup cannot adopt an unregistered or legacy run')
    const local = config.targets.find(target => target.kind === 'local')
    assert.equal(record.rootHash, rootHash(local.path), 'Registered approved root changed')
    const target = makePlan({ ...config, targets: [local], rcloneConfig: null }, runId).targets[0]
    await noLinks(target.run, fs)
    privateStat(await fs.lstat(target.run), true)
    const owner = await privateJson(child(target.run, 'owner.json'))
    assert.equal(owner.schema, 2, 'Legacy ownership is not sufficient for automated cleanup')
    assert.equal(owner.runId, runId)
    assert.equal(owner.nonce, record.nonce, 'Run/registry ownership differs')
    assert.equal(owner.rootHash, record.rootHash)
    assert.equal(owner.policy, this.policy.schema)
    return { registry, record, target, owner }
  }

  async audit(config, runId) {
    const { target, owner } = await this.#owned(config, runId)
    const tree = await inspectTree(target.run, { policy: this.policy })
    return { runId, bytes: tree.bytes, entries: tree.entries.length, private: true, selectedProviders: owner.selectedProviders }
  }

  async review(config, runId) {
    return this.#locked(async () => {
      const { registry, record, target, owner } = await this.#owned(config, runId)
      assert.equal(record.state, 'UNCERTAIN', 'Explicit review applies only to uncertain retained data')
      const report = await privateJson(child(target.run, 'report.json'))
      assert.equal(report.schema, 3)
      assert.equal(report.runId, runId)
      assert.ok(['PASS', 'FAIL', 'BLOCKED'].includes(report.status))
      assert.ok(Number.isFinite(Date.parse(report.finished)) && Date.parse(report.finished) <= Date.now())
      assert.equal(report.teardownDetails?.status, 'PASS', 'Unconfirmed teardown prevents review')
      assert.deepEqual(report.teardownDetails.steps.map(step => step.stage),
        ['session-close', 'driver-exit', 'native-driver-exit', 'candidate-exit'])
      assert.ok(report.teardownDetails.steps.every(step => step.status === 'PASS'))
      for (const [pid, start] of [[owner.runner?.pid, owner.runner?.start],
        [report.driverIdentity?.pid, report.driverIdentity?.startTime],
        [report.driverIdentity?.nativePid, report.driverIdentity?.nativeStartTime], [report.identity?.pid, report.identity?.startTime]]) {
        // Setup may fail before a process was launched; an incomplete recorded
        // identity is still rejected, rather than silently treated as gone.
        if (pid === undefined && start === undefined) continue
        assert.ok(await processGone(pid, start), 'A recorded owned process is still alive; refuse review')
      }
      const tree = await inspectTree(target.run, { policy: this.policy })
      const activePath = child(target.run, 'active.json')
      try {
        const active = await privateJson(activePath)
        assert.equal(active.runId, runId)
        assert.equal(active.nonce, record.nonce)
        await verifyEntry(activePath, tree.entries.find(entry => entry.path === 'active.json'))
        await fs.unlink(activePath)
      } catch (error) { if (error.code !== 'ENOENT') throw error }
      const audited = await inspectTree(target.run, { policy: this.policy })
      Object.assign(record, { bytes: audited.bytes, state: 'RETAINED', status: report.status })
      await this.#save(registry)
      return { runId, state: record.state, status: report.status, bytes: audited.bytes,
        private: true, scope: 'Recovery files and original report retained; only inactive marker cleared' }
    })
  }

  async #snapshot(config, runId, now) {
    const owned = await this.#owned(config, runId)
    const { target, owner, record } = owned
    try { await fs.lstat(child(target.run, 'link-policy.json')); throw new Error('Owned link fixtures require explicit manual retention review; automated cleanup is refused') }
    catch (error) { if (error.code !== 'ENOENT') throw error }
    assert.equal(record.state, 'RETAINED', 'Pending/uncertain runs preserve recovery data')
    assert.deepEqual(owner.selectedProviders, ['local'], 'Multi-provider recovery data and its local ownership anchor are retained')
    try { await fs.lstat(child(target.run, 'active.json')); throw new Error('Active run marker prevents cleanup') }
    catch (error) { if (error.code !== 'ENOENT') throw error }
    const report = await privateJson(child(target.run, 'report.json'))
    assert.equal(report.runId, runId)
    assert.equal(report.schema, 3)
    assert.equal(report.status, 'PASS', 'Failed/blocked runs preserve recovery data')
    assert.equal(report.build?.feature, 'native-test')
    assert.equal(report.teardownDetails?.status, 'PASS', 'Unconfirmed teardown prevents cleanup')
    assert.ok(report.cases.length && report.cases.every(item => item.status === 'PASS'))
    assert.deepEqual(report.teardownDetails.steps.map(step => step.stage),
      ['session-close', 'driver-exit', 'native-driver-exit', 'candidate-exit'])
    assert.ok(report.teardownDetails.steps.every(step => step.status === 'PASS'))
    const finished = Date.parse(report.finished)
    const created = Date.parse(owner.createdAt)
    assert.ok(Number.isFinite(finished) && Number.isFinite(created) && finished >= created && finished <= now,
      'Invalid/future run timestamps prevent cleanup')
    assert.ok(now - finished >= this.policy.successDays * 86400_000, 'Successful-run minimum retention period has not elapsed')
    for (const [pid, start] of [[owner.runner?.pid, owner.runner?.start],
      [report.driverIdentity?.pid, report.driverIdentity?.startTime],
      [report.driverIdentity?.nativePid, report.driverIdentity?.nativeStartTime], [report.identity?.pid, report.identity?.startTime]]) {
      assert.ok(await processGone(pid, start), 'A recorded owned process is still alive; refuse cleanup')
    }
    const tree = await inspectTree(target.run, { policy: this.policy })
    assert.ok(!tree.entries.some(entry => entry.type === 'socket'),
      'Runtime sockets preserve data: sidecar process ownership is not established for cleanup')
    return { ...owned, tree, planSha256: digest({ runId, nonce: owner.nonce, root: record.rootHash,
      finished: report.finished, policy: this.policy, tree: tree.sha256 }) }
  }

  async plan(config, runId, { now = Date.now() } = {}) {
    return this.#locked(async () => {
      const snapshot = await this.#snapshot(config, runId, now)
      return { runId, planSha256: snapshot.planSha256, bytes: snapshot.tree.bytes, entries: snapshot.tree.entries.length,
        eligible: true, scope: 'One verified local-only successful run; approved root remains' }
    })
  }

  async cleanup(config, runId, planSha256, { now = Date.now(), beforeDelete = async () => {} } = {}) {
    assert.match(planSha256, /^[a-f0-9]{64}$/, 'Explicit reviewed plan SHA-256 is required')
    return this.#locked(async () => {
      const snapshot = await this.#snapshot(config, runId, now)
      assert.equal(snapshot.planSha256, planSha256, 'Cleanup plan changed; no deletion, create and review a fresh plan')
      const { tree, target, registry, record } = snapshot
      // Do not delete control metadata until all other owned entries are gone.
      const control = new Set(['owner.json', 'report.json'])
      const rest = tree.entries.filter(entry => entry.path && !control.has(entry.path))
        .sort((a, b) => b.path.split('/').length - a.path.split('/').length || b.path.localeCompare(a.path))
      const entries = [...rest, ...tree.entries.filter(entry => control.has(entry.path)), tree.entries.find(entry => !entry.path)]
      let removed = 0
      try {
        for (const entry of entries) {
          const raw = entry.path ? `${target.run}/${entry.path}` : target.run
          await beforeDelete(entry)
          if (entry.path === 'owner.json') assert.deepEqual((await fs.readdir(target.run)).sort(),
            ['owner.json', 'report.json'], 'Unexpected remaining data; preserve control metadata')
          await verifyEntry(raw, entry)
          if (entry.type === 'directory') await fs.rmdir(raw)
          else await fs.unlink(raw)
          removed++
        }
        registry.runs = registry.runs.filter(run => run !== record)
        await this.#save(registry)
        return { status: 'PASS', runId, removed }
      } catch (error) {
        record.state = 'UNCERTAIN'
        await this.#save(registry)
        throw Object.assign(error, { failureKind: 'CLEANUP_UNCERTAIN', removed })
      }
    })
  }
}

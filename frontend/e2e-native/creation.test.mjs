import assert from 'node:assert/strict'
import { test } from 'node:test'
import { creation, creationManifest } from './creation.mjs'
import { child, makePlan, validateConfig, kinds, ownedPath } from './scope.mjs'
import { createReport, recordCase, finishReport, summarizeProviders } from './report.mjs'

const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind =>
  [kind, kind === 'cloud' ? 'rclone://Generated/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '00000000-0000-4000-8000-000000000000')

// Independent generated-files oracle; it supplements, never replaces native UI.
function model({ truncateOnRejection = false, failCancel = false } = {}) {
  const roots = plan.targets.map(target => target.files), items = new Map()
  let current, dialog, name, folder
  const calls = []
  const fixture = {
    mkdir: async path => { ownedPath(roots, path); assert.ok(!items.has(path)); items.set(path, null) },
    write: async (path, bytes) => { ownedPath(roots, path); assert.ok(!items.has(path)); items.set(path, bytes) },
    read: async path => items.get(path),
    snapshot: async path => [...items].filter(([key]) => key.slice(0, key.lastIndexOf('/')) === path)
      .map(([path, bytes]) => ({ path, kind: bytes === null ? 'dir' : 'file' })),
  }
  const ui = {
    navigate: async path => { ownedPath(roots, path); current = path },
    setView: async view => { assert.ok(['list', 'grid'].includes(view)) },
    beginCreation: async (base, kind) => { assert.equal(base, current); assert.ok(!dialog); dialog = true; folder = kind; name = '' },
    creationValue: async (kind, value) => { assert.ok(dialog); assert.equal(kind, folder); name = value },
    submitCreation: async (kind, options = {}) => {
      assert.ok(dialog); assert.equal(kind, folder)
      calls.push(['submit', current, kind, name, Boolean(options.error)])
      if (options.error) {
        if (truncateOnRejection) items.set(child(current, 'existing.txt'), '')
      } else {
        const path = child(current, name); assert.ok(!items.has(path)); items.set(path, kind ? null : ''); dialog = false
      }
    },
    cancelCreation: async kind => { assert.ok(dialog); assert.equal(kind, folder); calls.push(['cancel']);
      if (failCancel) throw new Error('synthetic cancellation failure'); dialog = false },
    creationFocus: async base => { assert.equal(base, current); assert.ok(!dialog) },
  }
  return { fixture, ui, calls }
}

test('creation records every provider, both kinds, rejected names, preservation, cancellation and grid recovery', async () => {
  const report = createReport(plan, kinds, creationManifest(plan)), { fixture, ui, calls } = model()
  await creation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action))
  finishReport(report); summarizeProviders(report)
  assert.equal(report.status, 'PASS')
  assert.ok(report.cases.every(item => item.parts.length === 24 && item.parts.every(part => part.status === 'PASS' && part.ui === 'ACKNOWLEDGED')))
  for (const target of plan.targets) {
    assert.equal(report.providers[target.kind].status, 'PASS')
    for (const kind of [false, true]) for (const name of ['', '   ', 'invalid/name', 'invalid\\name', '.', '..', 'existing.txt', 'existing-folder']) {
      assert.ok(calls.some(item => item[0] === 'submit' && item[1].startsWith(target.files) && item[2] === kind && item[3] === name && item[4]))
    }
  }
})

test('rejected creation that truncates an existing fixture fails independent verification without retry', async () => {
  const report = createReport(plan, kinds, creationManifest(plan)), { fixture, ui, calls } = model({ truncateOnRejection: true })
  await assert.rejects(creation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action)), /never be truncated/)
  assert.equal(report.cases[0].status, 'FAIL')
  assert.equal(report.cases[0].failureKind, 'RESULT_MISMATCH')
  assert.equal(report.cases[0].parts.at(-1).ui, 'ACKNOWLEDGED')
  assert.equal(calls.filter(item => item[0] === 'submit' && item[4]).length, 1)
  assert.ok(report.cases.slice(1).every(item => item.status === 'NOT_RUN'))
})

test('cancellation failure preserves completed creation parts and leaves later providers unrun', async () => {
  const report = createReport(plan, kinds, creationManifest(plan)), { fixture, ui, calls } = model({ failCancel: true })
  await assert.rejects(creation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action)), /cancellation failure/)
  assert.deepEqual(report.cases[0].parts.map(part => part.status), ['PASS', 'PASS', 'BLOCKED'])
  assert.equal(calls.filter(item => item[0] === 'cancel').length, 1)
  assert.ok(report.cases.slice(1).every(item => item.status === 'NOT_RUN'))
})

test('a failed parallel verification waits for the other fixture read before recording failure', async () => {
  const report = createReport(plan, kinds, creationManifest(plan)), { fixture, ui } = model()
  const snapshot = fixture.snapshot, read = fixture.read
  let finishRead, readStarted, settled = false, ended = false
  const started = new Promise(resolve => { readStarted = resolve })
  fixture.snapshot = async path => {
    if (path.endsWith('/creation')) throw Object.assign(new Error('synthetic fixture snapshot failure'), { failureKind: 'FIXTURE_IO' })
    return snapshot(path)
  }
  fixture.read = async path => {
    if (path.endsWith('/existing.txt')) {
      readStarted()
      await new Promise(resolve => { finishRead = resolve })
      settled = true
    }
    return read(path)
  }
  const running = creation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action))
    .finally(() => { ended = true })
  const rejected = assert.rejects(running, /fixture snapshot failure/)
  await started
  assert.equal(ended, false)
  finishRead(); await rejected
  assert.equal(settled, true)
  assert.equal(report.cases[0].status, 'BLOCKED')
  assert.equal(report.cases[0].parts[0].ui, 'ACKNOWLEDGED')
})

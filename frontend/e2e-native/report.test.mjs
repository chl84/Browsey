import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { validateConfig, makePlan, kinds } from './scope.mjs'
import { foundationManifest } from './cases.mjs'
import { createReport, recordSetup, recordCase, recordPart, finishReport, summarizeProviders } from './report.mjs'
import { applyTeardown } from './lifecycle.mjs'
import { createLocalSession } from './fixtures.mjs'

const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind =>
  [kind, kind === 'cloud' ? 'rclone://Test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '00000000-0000-4000-8000-000000000000')
const manifest = foundationManifest(plan)
const fresh = (items = manifest) => createReport(plan, kinds, items)
const metadata = id => manifest.find(item => item.id === id)
const injected = (failureKind, code) => Object.assign(new Error('synthetic failure'), { failureKind, code })

test('each part persists started and completed outcomes before its whole case finishes', async () => {
  const item = { ...metadata('copy-within-local'), partIds: ['first', 'second', 'later'] }
  const report = fresh([item]), snapshots = []
  const persist = async () => snapshots.push(JSON.parse(JSON.stringify(report.cases[0])))
  await assert.rejects(recordCase(report, item, async result => {
    await recordPart(result, 'first', async () => {})
    await recordPart(result, 'second', async () => { throw injected('APP_REPORTED_ERROR') })
  }, { persist }), /synthetic failure/)
  assert.ok(snapshots.some(c => c.status === 'RUNNING' && c.parts[0].status === 'RUNNING'))
  assert.ok(snapshots.some(c => c.status === 'RUNNING' && c.parts[0].status === 'PASS' && c.parts[1].status === 'NOT_RUN'))
  assert.ok(snapshots.some(c => c.status === 'RUNNING' && c.parts[1].status === 'FAIL'))
  assert.equal(snapshots.at(-1).status, 'FAIL')
  assert.equal(snapshots.at(-1).parts[2].status, 'NOT_RUN')
  assert.ok(!JSON.stringify(report).includes('partPersistence'))
})

test('all five providers declare directional requirements without assuming support or N/A', () => {
  const report = fresh()
  const outward = report.cases.find(item => item.id === 'move-local-mobile')
  assert.ok(outward.requirements.local.includes('source-removal'))
  assert.ok(!outward.requirements.mobile.includes('source-removal'))
  assert.ok(outward.requirements.mobile.includes('file-write'))
  const inward = report.cases.find(item => item.id === 'move-mobile-local')
  assert.ok(inward.requirements.mobile.includes('source-removal'))
  assert.ok(report.cases.every(item => item.status === 'NOT_RUN'))
  summarizeProviders(report)
  for (const kind of kinds) {
    assert.ok(Object.keys(report.providers[kind].capabilities).length)
    assert.ok(Object.values(report.providers[kind].capabilities).every(item => item.status === 'NOT_RUN'))
  }
})

test('excluded and unconfigured providers remain distinct and cannot receive acceptance', () => {
  const localPlan = { ...plan, targets: plan.targets.filter(item => item.kind === 'local'), routes: [] }
  const report = createReport(localPlan, ['local', 'mobile'], foundationManifest(localPlan))
  summarizeProviders(report)
  assert.equal(report.providers.mobile.status, 'DEFERRED')
  assert.equal(report.providers.usb.status, 'NOT_CONFIGURED')
  assert.deepEqual(report.providers.mobile.capabilities, {})
  assert.ok(report.cases.every(item => item.providers.length === 1 && item.providers[0] === 'local'))
})

test('setup failure is recorded before UI, leaves all cases NOT_RUN and forbids setup retry', async () => {
  const report = fresh()
  let attempts = 0
  const action = () => { attempts++; throw injected('FIXTURE_IO') }
  const stage = { id: 'owned-run-mobile', providers: ['mobile'] }
  let error
  try { await recordSetup(report, stage, action) } catch (observed) { error = observed }
  finishReport(report, error)
  summarizeProviders(report)
  assert.equal(report.status, 'BLOCKED')
  assert.equal(report.failureOrigin, 'harness')
  assert.equal(report.providers.mobile.setup[0].status, 'BLOCKED')
  assert.ok(report.cases.every(item => item.status === 'NOT_RUN'))
  await assert.rejects(recordSetup(report, stage, action), /must not be retried/)
  assert.equal(attempts, 1)
})

test('partial provider setup retains an owned local report before a later run creation fails', async t => {
  const temporary = await fs.mkdtemp('/tmp/n')
  t.after(() => fs.rm(temporary, { recursive: true }))
  await fs.mkdir(`${temporary}/ai_agent_testfolder`)
  await fs.mkdir(`${temporary}/usb`)
  await fs.mkdir(`${temporary}/usb/ai_agent_testfolder`)
  const config = validateConfig({ schema: 1, targets: { local: `${temporary}/ai_agent_testfolder`,
    usb: `${temporary}/usb/ai_agent_testfolder` } })
  const partial = makePlan(config, randomUUID())
  const report = createReport(partial, ['local', 'usb'], foundationManifest(partial))
  const local = partial.targets[0]
  let owned = false
  const persist = async () => {
    summarizeProviders(report)
    if (owned) await fs.writeFile(`${local.run}/report.json`, JSON.stringify(report), { mode: 0o600 })
  }
  let error
  try {
    await createLocalSession(partial, config, { onOwned: async () => {
      owned = true
      assert.equal(JSON.parse(await fs.readFile(`${local.run}/owner.json`)).runId, partial.runId)
      await persist()
    }, step: (metadata, action) => recordSetup(report, metadata, () => {
      if (metadata.id === 'owned-run-usb') throw injected('FIXTURE_IO')
      return action()
    }, persist) })
  } catch (observed) { error = observed }
  assert.ok(error, 'Injected later provider setup must fail')
  finishReport(report, error)
  await persist()
  const retained = JSON.parse(await fs.readFile(`${local.run}/report.json`))
  assert.equal(retained.status, 'BLOCKED')
  assert.equal(retained.providers.usb.setup.find(item => item.id === 'owned-run-usb').status, 'BLOCKED')
  assert.equal(retained.providers.local.setup.find(item => item.id === 'owned-run-local').status, 'PASS')
  assert.ok(retained.cases.every(item => item.status === 'NOT_RUN'))
  await assert.rejects(fs.lstat(partial.targets[1].run), error => error.code === 'ENOENT')
})

test('partial transfer preserves verified file, blocked directory and later NOT_RUN cases without retry', async () => {
  const report = fresh()
  const item = metadata('copy-local-mobile')
  let sent = 0
  let error
  try {
    await recordCase(report, item, async result => {
      for (const id of ['file', 'directory']) await recordPart(result, id, async part => {
        result.phase = 'ui'; part.ui = 'STARTED'; sent++; part.ui = 'ACKNOWLEDGED'
        result.phase = 'verification'
        if (id === 'directory') throw injected('FIXTURE_IO')
      })
    })
  } catch (observed) { error = observed }
  finishReport(report, error)
  summarizeProviders(report)
  const result = report.cases.find(entry => entry.id === item.id)
  assert.equal(result.status, 'BLOCKED')
  assert.deepEqual(result.parts.map(({ status, ui }) => ({ status, ui })), [
    { status: 'PASS', ui: 'ACKNOWLEDGED' }, { status: 'BLOCKED', ui: 'ACKNOWLEDGED' }])
  for (const kind of ['local', 'mobile']) {
    assert.equal(report.providers[kind].counts.BLOCKED, 1)
    assert.ok(report.providers[kind].counts.NOT_RUN > 0)
    assert.equal(report.providers[kind].capabilities['ui-copy'].status, 'BLOCKED')
  }
  await assert.rejects(recordCase(report, item, async () => { sent++ }), /must not be retried/)
  await assert.rejects(recordPart(result, 'directory', async () => { sent++ }), /must not be retried/)
  assert.equal(sent, 2)
})

test('first-part failure retains an explicit NOT_RUN directory with no sent action', async () => {
  const report = fresh()
  await assert.rejects(recordCase(report, metadata('move-within-mobile'), async result => {
    await recordPart(result, 'file', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; throw injected('APP_REPORTED_ERROR')
    })
  }))
  const result = report.cases.find(item => item.id === 'move-within-mobile')
  assert.equal(result.parts[0].status, 'FAIL')
  assert.deepEqual(result.parts[1], { id: 'directory', status: 'NOT_RUN', ui: 'NOT_SENT' })
})

test('app-reported error, result mismatch, fixture I/O and unknown UI error have distinct evidence', async () => {
  for (const [error, phase, status, kind, origin] of [
    [injected('APP_REPORTED_ERROR'), 'ui', 'FAIL', 'APP_REPORTED_ERROR', 'app-reported'],
    [injected(undefined, 'ERR_ASSERTION'), 'verification', 'FAIL', 'RESULT_MISMATCH', 'candidate-result'],
    [injected(undefined, 'EIO'), 'verification', 'BLOCKED', 'FIXTURE_IO', 'harness'],
    [injected(), 'ui', 'BLOCKED', 'UNCLASSIFIED_UI_OR_RESULT', 'undetermined'],
    [injected('DRIVER_EXITED'), 'ownership', 'BLOCKED', 'DRIVER_EXITED', 'harness'],
  ]) {
    const report = fresh()
    await assert.rejects(recordCase(report, metadata('create-local'), async result => { result.phase = phase; throw error }))
    finishReport(report, error)
    assert.equal(report.status, status)
    assert.equal(report.failureKind, kind)
    assert.equal(report.failureOrigin, origin)
  }
})

test('a later provider failure preserves completed unrelated provider cases and capabilities', async () => {
  const report = fresh([metadata('create-local'), metadata('create-mobile')])
  await recordCase(report, metadata('create-local'), async () => {})
  const error = injected('APP_REPORTED_ERROR')
  await assert.rejects(recordCase(report, metadata('create-mobile'), async () => { throw error }))
  finishReport(report, error)
  summarizeProviders(report)
  assert.equal(report.status, 'FAIL')
  assert.equal(report.providers.local.status, 'PASS')
  assert.equal(report.providers.mobile.status, 'FAIL')
  assert.deepEqual(report.providers.local.capabilities['file-create'].passedBy, ['create-local'])
})

test('undeclared cases and mismatched provider directions stop before any action', async () => {
  const report = fresh()
  let actions = 0
  await assert.rejects(recordCase(report, { id: 'not-declared', providers: ['local'] }, () => { actions++ }))
  await assert.rejects(recordCase(report, { id: 'copy-local-mobile', providers: ['mobile', 'local'] }, () => { actions++ }))
  assert.equal(actions, 0)
  assert.throws(() => fresh([{ id: 'unknown-case', providers: ['local'] }]), /declare requirements/)
  assert.throws(() => fresh([metadata('create-local'), metadata('create-local')]), /Unique declared/)
})

test('uncertain teardown blocks overall/provider PASS while retaining verified cases', async () => {
  const report = fresh([metadata('create-local')])
  await recordCase(report, metadata('create-local'), async () => {})
  finishReport(report)
  applyTeardown(report, { status: 'BLOCKED', steps: [{ stage: 'session-close', status: 'BLOCKED' }] })
  summarizeProviders(report)
  assert.equal(report.status, 'BLOCKED')
  assert.equal(report.providers.local.status, 'BLOCKED')
  assert.equal(report.cases[0].status, 'PASS')
  assert.deepEqual(report.providers.local.capabilities['file-create'].passedBy, ['create-local'])
})

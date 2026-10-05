import assert from 'node:assert/strict'
import { test } from 'node:test'
import { validateConfig, makePlan } from './scope.mjs'
import { foundation, foundationManifest } from './cases.mjs'
import { Fixtures, localEntryExists } from './fixtures.mjs'

const id = '00000000-0000-4000-8000-000000000000'
const plan = makePlan(validateConfig({ schema: 1, targets: { local: '/approved/ai_agent_testfolder',
  usb: '/usb/ai_agent_testfolder' } }), id)

// Orchestration regressions only: this in-memory model is NOT native acceptance.
function model(corrupt = false) {
  const items = new Map()
  let lastCopy
  const relocate = (source, dest, move) => {
    for (const [raw, value] of [...items]) {
      if (raw === source || raw.startsWith(`${source}/`)) {
        items.set(`${dest}${raw.slice(source.length)}`, corrupt && value !== null ? 'corrupt' : value)
        if (move) items.delete(raw)
      }
    }
  }
  const fixture = {
    mkdir: async raw => { assert.ok(!items.has(raw)); items.set(raw, null) },
    write: async (raw, content = 'Browsey native fixture: generated, non-personal data.\n'.repeat(64)) => {
      assert.ok(!items.has(raw)); items.set(raw, content)
    },
    exists: async raw => items.has(raw),
    read: async raw => { assert.ok(items.has(raw)); return items.get(raw) },
  }
  const ui = {
    create: async (base, name, folder) => folder ? fixture.mkdir(`${base}/${name}`) : fixture.write(`${base}/${name}`, ''),
    rename: async (raw, name) => relocate(raw, `${raw.slice(0, raw.lastIndexOf('/'))}/${name}`, true),
    remove: async (raw, cancel) => { if (!cancel) items.delete(raw) },
    transfer: async (raw, dest, move) => {
      const target = `${dest}/${raw.split('/').at(-1)}`
      relocate(raw, target, move)
      lastCopy = [raw, target]
    },
    idle: async () => {},
    enterPath: async () => {},
    chord: async key => {
      if (key === 'z') items.delete(lastCopy[1])
      else if (key === 'y') relocate(...lastCopy, false)
      else assert.fail('Unexpected shortcut')
    },
  }
  return { fixture, ui }
}

test('shared cases cover within-provider copy/move, both hub directions and local undo', async () => {
  const { fixture, ui } = model()
  const names = []
  const declared = []
  await foundation(plan, fixture, ui, async (name, action, metadata) => { await action(); names.push(name); declared.push({name, ...metadata}) })
  assert.deepEqual(declared, foundationManifest(plan))
  assert.equal(names.length, 16)
  for (const name of ['copy-within-local', 'move-within-usb', 'copy-local-usb', 'copy-usb-local',
    'move-local-usb', 'move-usb-local', 'local: copy undo/redo']) assert.ok(names.includes(name))
})

test('setup failure belongs to the declared case and stops before UI mutations', async () => {
  const { fixture, ui } = model()
  let uiCalls = 0
  ui.create = async () => { uiCalls++ }
  fixture.mkdir = async () => { throw new Error('synthetic setup denial') }
  const results = []
  await assert.rejects(foundation(plan, fixture, ui, async (name, action, metadata) => {
    const result = { name, ...metadata }
    results.push(result)
    await action(result)
  }), /synthetic setup denial/)
  assert.equal(uiCalls, 0)
  assert.equal(results.length, 1)
  assert.equal(results[0].phase, 'setup')
  assert.equal(results[0].id, 'input-local')
  assert.deepEqual(results[0].providers, ['local'])
})

test('redo waits for undo acknowledgement and the reconciled absent row', async () => {
  const { fixture, ui } = model()
  const chord = ui.chord
  let undoPending = false
  let redoAcknowledged = false
  ui.chord = async key => {
    if (key === 'z') undoPending = true
    if (key === 'y') assert.equal(undoPending, false, 'Do not lose redo while history is busy')
    await chord(key)
  }
  ui.idle = async expected => {
    if (undoPending) {
      assert.equal(expected?.toast, 'Undo')
      assert.match(expected.absentPath, /\/undo-target\/undo.txt$/)
      undoPending = false
    } else if (expected?.toast === 'Redo') {
      assert.match(expected.resultPath, /\/undo-target\/undo.txt$/)
      redoAcknowledged = true
    }
  }
  await foundation(plan, fixture, ui, async (_name, action) => action())
  assert.equal(redoAcknowledged, true)
})

test('independent byte verification rejects UI-reported success with corrupt output', async () => {
  const { fixture, ui } = model(true)
  await assert.rejects(foundation(plan, fixture, ui, async (_name, action) => action()))
})

test('fixture I/O rejects every outside path before touching real filesystem or rclone', async () => {
  const fixture = new Fixtures(plan, {})
  for (const raw of ['/personal/file', `${plan.targets[0].files}-sibling/file`, 'rclone://Other/personal']) {
    for (const method of ['read', 'exists', 'mkdir', 'write', 'snapshot']) await assert.rejects(fixture[method](raw))
  }
  await assert.rejects(fixture.ensureCloudRoot({ path: 'rclone://Other/personal' }))
})

test('move verification rejects stale positive metadata and never lists above its owned root', async () => {
  const roots = ['/owned/run/files']
  const lists = []
  const filesystem = { lstat: async () => ({ isSymbolicLink: () => false }),
    readdir: async raw => { lists.push(raw); return ['unrelated-fixture.txt'] } }
  assert.equal(await localEntryExists(roots, '/owned/run/files/source/moved.txt', filesystem), false)
  assert.deepEqual(lists, ['/owned/run/files/source'])
  assert.equal(await localEntryExists(roots, roots[0], filesystem), true)
  assert.equal(lists.length, 1, 'Root checks must not list the parent outside this run')
  await assert.rejects(localEntryExists(roots, '/personal/file', filesystem))
  assert.equal(lists.length, 1)
  filesystem.readdir = async () => ['moved.txt']
  assert.equal(await localEntryExists(roots, '/owned/run/files/source/moved.txt', filesystem), true)
})

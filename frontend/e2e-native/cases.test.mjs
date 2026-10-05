import assert from 'node:assert/strict'
import { test } from 'node:test'
import { validateConfig, makePlan } from './scope.mjs'
import { foundation } from './cases.mjs'
import { Fixtures } from './fixtures.mjs'

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
  await foundation(plan, fixture, ui, async (name, action) => { await action(); names.push(name) })
  assert.equal(names.length, 15)
  for (const name of ['copy-within-local', 'move-within-usb', 'copy-local-usb', 'copy-usb-local',
    'move-local-usb', 'move-usb-local', 'local: copy undo/redo']) assert.ok(names.includes(name))
})

test('independent byte verification rejects UI-reported success with corrupt output', async () => {
  const { fixture, ui } = model(true)
  await assert.rejects(foundation(plan, fixture, ui, async (_name, action) => action()))
})

test('fixture I/O rejects every outside path before touching real filesystem or rclone', async () => {
  const fixture = new Fixtures(plan, {})
  for (const raw of ['/personal/file', `${plan.targets[0].files}-sibling/file`, 'rclone://Other/personal']) {
    for (const method of ['read', 'exists', 'mkdir', 'write']) await assert.rejects(fixture[method](raw))
  }
  await assert.rejects(fixture.ensureCloudRoot({ path: 'rclone://Other/personal' }))
})

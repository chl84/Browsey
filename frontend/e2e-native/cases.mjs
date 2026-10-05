import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { payload } from './fixtures.mjs'
import { recordPart } from './report.mjs'

export function foundationManifest(plan) {
  const cases = [{ id: 'input-local', name: 'local: exact path/name input after modifier release', providers: ['local'] }]
  for (const target of plan.targets) {
    for (const [operation, name] of [['create', 'new folder/file'], ['rename', 'file/folder rename'], ['delete', 'permanent delete/cancel']]) {
      cases.push({ id: `${operation}-${target.kind}`, name: `${target.kind}: ${name}`, providers: [target.kind] })
    }
    for (const operation of ['copy', 'move']) {
      const id = `${operation}-within-${target.kind}`
      cases.push({ id, name: id, providers: [target.kind] })
    }
  }
  for (const { from, to } of plan.routes) for (const operation of ['copy', 'move']) {
    const id = `${operation}-${from}-${to}`
    cases.push({ id, name: id, providers: [from, to] })
  }
  cases.push({ id: 'undo-copy-local', name: 'local: copy undo/redo', providers: ['local'] })
  return cases
}

// These cases express the same outcomes for every provider. Device semantics
// such as chmod/trash are not silently assumed to be portable.
async function transferCase(source, target, label, move, fixture, ui, record) {
  const from = child(source.files, `${label}-source`)
  const to = child(target.files, `${label}-target`)
  await record(label, async (result = {}) => {
    result.phase = 'setup'
    await fixture.mkdir(from)
    await fixture.mkdir(to)
    await fixture.write(child(from, 'sample.txt'))
    await fixture.mkdir(child(from, 'tree'))
    await fixture.write(child(child(from, 'tree'), 'nested.txt'))
    for (const leaf of ['sample.txt', 'tree']) {
      await recordPart(result, leaf === 'tree' ? 'directory' : 'file', async part => {
        result.phase = 'ui'
        part.ui = 'STARTED'
        await ui.transfer(child(from, leaf), to, move)
        part.ui = 'ACKNOWLEDGED'
        result.phase = 'verification'
        const relative = leaf === 'tree' ? 'tree/nested.txt' : leaf
        assert.equal(await fixture.read(`${to}/${relative}`), payload)
        assert.equal(await fixture.exists(child(from, leaf)), !move)
      })
    }
  }, { id: label, providers: [...new Set([source.kind, target.kind])] })
}

export async function foundation(plan, fixture, ui, record) {
  const local = plan.targets.find(target => target.kind === 'local')
  await record('local: exact path/name input after modifier release', async (result = {}) => {
    result.phase = 'setup'
    const base = child(local.files, 'input_æøå')
    const source = child(base, 'source_æøå.txt')
    const renamed = child(base, 'renamed_æøå.txt')
    await fixture.mkdir(base)
    await fixture.write(source)
    result.phase = 'ui'
    await ui.remove(source, true)
    // Slash, underscore and Unicode must remain exact after Shift+Delete.
    await ui.enterPath(base)
    await ui.create(base, 'created_æøå.txt', false)
    await ui.rename(source, 'renamed_æøå.txt')
    result.phase = 'verification'
    assert.equal(await fixture.read(child(base, 'created_æøå.txt')), '')
    assert.equal(await fixture.read(renamed), payload)
    assert.ok(!await fixture.exists(source))
  }, { id: 'input-local', providers: ['local'] })
  for (const target of plan.targets) {
    const base = child(target.files, 'basic')
    await record(`${target.kind}: new folder/file`, async (result = {}) => {
      result.phase = 'setup'
      await fixture.mkdir(base)
      await fixture.write(child(base, 'source.txt'))
      result.phase = 'ui'
      await ui.create(base, 'created-folder', true)
      result.phase = 'verification'
      assert.ok(await fixture.exists(child(base, 'created-folder')))
      result.phase = 'ui'
      await ui.create(base, 'created.txt', false)
      result.phase = 'verification'
      assert.equal(await fixture.read(child(base, 'created.txt')), '')
    }, { id: `create-${target.kind}`, providers: [target.kind] })
    await record(`${target.kind}: file/folder rename`, async (result = {}) => {
      await ui.rename(child(base, 'source.txt'), 'renamed.txt')
      result.phase = 'verification'
      assert.ok(!await fixture.exists(child(base, 'source.txt')))
      assert.equal(await fixture.read(child(base, 'renamed.txt')), payload)
      result.phase = 'ui'
      await ui.rename(child(base, 'created-folder'), 'renamed-folder')
      result.phase = 'verification'
      assert.ok(!await fixture.exists(child(base, 'created-folder')))
      assert.ok(await fixture.exists(child(base, 'renamed-folder')))
    }, { id: `rename-${target.kind}`, providers: [target.kind] })
    await record(`${target.kind}: permanent delete/cancel`, async (result = {}) => {
      const raw = child(base, 'renamed.txt')
      await ui.remove(raw, true)
      result.phase = 'verification'
      assert.equal(await fixture.read(raw), payload)
      result.phase = 'ui'
      await ui.remove(raw)
      result.phase = 'verification'
      assert.ok(!await fixture.exists(raw))
      result.phase = 'ui'
      await ui.remove(child(base, 'renamed-folder'))
      result.phase = 'verification'
      assert.ok(!await fixture.exists(child(base, 'renamed-folder')))
    }, { id: `delete-${target.kind}`, providers: [target.kind] })
    for (const move of [false, true]) {
      await transferCase(target, target, `${move ? 'move' : 'copy'}-within-${target.kind}`, move, fixture, ui, record)
    }
  }
  for (const route of plan.routes) {
    const source = plan.targets.find(target => target.kind === route.from)
    const target = plan.targets.find(target => target.kind === route.to)
    for (const move of [false, true]) {
      const label = `${move ? 'move' : 'copy'}-${source.kind}-${target.kind}`
      await transferCase(source, target, label, move, fixture, ui, record)
    }
  }
  const from = child(local.files, 'undo-source')
  const to = child(local.files, 'undo-target')
  await record('local: copy undo/redo', async (result = {}) => {
    result.phase = 'setup'
    await fixture.mkdir(from)
    await fixture.mkdir(to)
    await fixture.write(child(from, 'undo.txt'))
    result.phase = 'ui'
    await ui.transfer(child(from, 'undo.txt'), to)
    result.phase = 'verification'
    assert.equal(await fixture.read(child(to, 'undo.txt')), payload)
    result.phase = 'ui'
    await ui.chord('z')
    await ui.idle({ toast: 'Undo', absentPath: child(to, 'undo.txt') })
    result.phase = 'verification'
    assert.ok(!await fixture.exists(child(to, 'undo.txt')))
    result.phase = 'ui'
    await ui.chord('y')
    await ui.idle({ toast: 'Redo', resultPath: child(to, 'undo.txt') })
    result.phase = 'verification'
    assert.equal(await fixture.read(child(to, 'undo.txt')), payload)
    assert.equal(await fixture.read(child(from, 'undo.txt')), payload)
  }, { id: 'undo-copy-local', providers: ['local'] })
}

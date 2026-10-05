import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { payload } from './fixtures.mjs'

// These cases express the same outcomes for every provider. Device semantics
// such as chmod/trash are not silently assumed to be portable.
async function transferCase(source, target, label, move, fixture, ui, record) {
  const from = child(source.files, `${label}-source`)
  const to = child(target.files, `${label}-target`)
  await fixture.mkdir(from)
  await fixture.mkdir(to)
  await fixture.write(child(from, 'sample.txt'))
  await fixture.mkdir(child(from, 'tree'))
  await fixture.write(child(child(from, 'tree'), 'nested.txt'))
  await record(label, async () => {
    for (const leaf of ['sample.txt', 'tree']) {
      await ui.transfer(child(from, leaf), to, move)
      const relative = leaf === 'tree' ? 'tree/nested.txt' : leaf
      assert.equal(await fixture.read(`${to}/${relative}`), payload)
      assert.equal(await fixture.exists(child(from, leaf)), !move)
    }
  })
}

export async function foundation(plan, fixture, ui, record) {
  for (const target of plan.targets) {
    const base = child(target.files, 'basic')
    await fixture.mkdir(base)
    await fixture.write(child(base, 'source.txt'))
    await record(`${target.kind}: new folder/file`, async () => {
      await ui.create(base, 'created-folder', true)
      assert.ok(await fixture.exists(child(base, 'created-folder')))
      await ui.create(base, 'created.txt', false)
      assert.equal(await fixture.read(child(base, 'created.txt')), '')
    })
    await record(`${target.kind}: file/folder rename`, async () => {
      await ui.rename(child(base, 'source.txt'), 'renamed.txt')
      assert.ok(!await fixture.exists(child(base, 'source.txt')))
      assert.equal(await fixture.read(child(base, 'renamed.txt')), payload)
      await ui.rename(child(base, 'created-folder'), 'renamed-folder')
      assert.ok(!await fixture.exists(child(base, 'created-folder')))
      assert.ok(await fixture.exists(child(base, 'renamed-folder')))
    })
    await record(`${target.kind}: permanent delete/cancel`, async () => {
      const raw = child(base, 'renamed.txt')
      await ui.remove(raw, true)
      assert.equal(await fixture.read(raw), payload)
      await ui.remove(raw)
      assert.ok(!await fixture.exists(raw))
      await ui.remove(child(base, 'renamed-folder'))
      assert.ok(!await fixture.exists(child(base, 'renamed-folder')))
    })
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
  const local = plan.targets.find(target => target.kind === 'local')
  const from = child(local.files, 'undo-source')
  const to = child(local.files, 'undo-target')
  await fixture.mkdir(from)
  await fixture.mkdir(to)
  await fixture.write(child(from, 'undo.txt'))
  await record('local: copy undo/redo', async () => {
    await ui.transfer(child(from, 'undo.txt'), to)
    assert.equal(await fixture.read(child(to, 'undo.txt')), payload)
    await ui.chord('z')
    await ui.idle()
    assert.ok(!await fixture.exists(child(to, 'undo.txt')))
    await ui.chord('y')
    await ui.idle()
    assert.equal(await fixture.read(child(to, 'undo.txt')), payload)
    assert.equal(await fixture.read(child(from, 'undo.txt')), payload)
  })
}

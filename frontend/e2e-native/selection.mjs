import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { recordPart } from './report.mjs'

export function selectionManifest(plan) {
  return [...plan.targets.map(target => ({ id: `selection-${target.kind}`,
    name: `${target.kind}: selection and exact copy in list/grid`, providers: [target.kind] })),
  { id: 'selection-virtual-local', name: 'local: offscreen selection in virtualized list', providers: ['local'] }]
}

const stepper = result => (id, action) => recordPart(result, id, async part => {
  result.phase = 'ui'; part.ui = 'STARTED'
  await action(part)
  part.ui = 'ACKNOWLEDGED'
})

export async function verifySelectedCopy(fixture, dest, paths, bytes) {
  const expected = paths.map(path => child(dest, path.split('/').at(-1)))
  const snapshot = await fixture.snapshot(dest, { maxChildren: 256 })
  assert.deepEqual(snapshot.map(entry => entry.path).sort(), expected.sort(), 'Copy must contain exactly the intended unique entries')
  for (const path of paths) assert.equal(await fixture.read(child(dest, path.split('/').at(-1))), bytes.get(path), 'Selected copy bytes must match')
}

export async function selection(plan, fixture, ui, record) {
  for (const target of plan.targets) {
    await record(`${target.kind}: owned list/grid selection`, async (result = {}) => {
      result.phase = 'setup'
      const base = child(target.files, 'selection'), nested = child(base, 'nested')
      const paths = Array.from({ length: 5 }, (_, i) => child(base, `file-${i}.txt`))
      const bytes = new Map(paths.map((path, i) => [path, `Generated selection file ${i}\n`]))
      await fixture.mkdir(base); await fixture.mkdir(nested)
      for (const [path, value] of bytes) await fixture.write(path, value)
      const visible = [nested, ...paths]
      const step = stepper(result)
      for (const view of ['list', 'grid']) {
        const subsetDest = child(target.files, `selection-${view}-subset`)
        const allDest = child(target.files, `selection-${view}-all`)
        result.phase = 'setup'; await fixture.mkdir(subsetDest); await fixture.mkdir(allDest)
        await step(`${view}-single-ctrl-toggle`, async () => {
          await ui.navigate(base); await ui.setView('list'); await ui.sort('Name', 'asc'); await ui.hidden(false)
          await ui.setView(view); await ui.listing(base, view, visible)
          await ui.select(paths[0]); await ui.selection(base, [paths[0]])
          await ui.modifiedSelect(paths[2], 'Control'); await ui.selection(base, [paths[0], paths[2]])
          await ui.modifiedSelect(paths[0], 'Control'); await ui.selection(base, [paths[2]])
        })
        await step(`${view}-shift-and-arrow-range`, async () => {
          await ui.select(paths[0]); await ui.modifiedSelect(paths[3], 'Shift')
          await ui.selection(base, paths.slice(0, 4))
          await ui.selectionKey(view === 'list' ? 'ArrowUp' : 'ArrowLeft', 'Shift')
          await ui.selection(base, paths.slice(0, 3))
          await ui.selectionKey(view === 'list' ? 'ArrowDown' : 'ArrowRight')
          await ui.selection(base, [paths[3]])
        })
        await step(`${view}-empty-space-and-escape`, async () => {
          await ui.emptySpace(base); await ui.selection(base, [])
          await ui.select(paths[1]); await ui.selectionKey('Escape'); await ui.selection(base, [])
        })
        await step(`${view}-navigation-clears-and-restores`, async () => {
          await ui.select(paths[0]); await ui.modifiedSelect(paths[2], 'Control')
          await ui.openFolder(nested); await ui.selection(nested, [])
          await ui.history('back', base); await ui.listing(base, view, visible)
          // Opening the folder deliberately selects it; history restores that folder's selection.
          await ui.selection(base, [nested], { directories: [nested] })
          await ui.select(paths[0]); await ui.modifiedSelect(paths[2], 'Control')
          await ui.selection(base, [paths[0], paths[2]])
        })
        await step(`${view}-exact-subset-copy`, async part => {
          await ui.copySelection(subsetDest, [paths[0], paths[2]])
          part.ui = 'ACKNOWLEDGED'
          result.phase = 'verification'; await verifySelectedCopy(fixture, subsetDest, [paths[0], paths[2]], bytes)
        })
        await step(`${view}-filtered-select-all-copy`, async part => {
          await ui.navigate(base); await ui.filter(base, paths[0], '.txt')
          await ui.listing(base, view, paths)
          // Return keyboard focus to the collection without leaving filter mode.
          await ui.select(paths[1]); await ui.chord('a'); await ui.selection(base, paths)
          await ui.copySelection(allDest, paths)
          part.ui = 'ACKNOWLEDGED'
          result.phase = 'verification'; await verifySelectedCopy(fixture, allDest, paths, bytes)
          result.phase = 'ui'; await ui.exitQuery(allDest)
        })
      }
      result.phase = 'verification'
      for (const [path, value] of bytes) assert.equal(await fixture.read(path), value)
      assert.ok(await fixture.exists(nested))
      result.phase = 'ui'; await ui.navigate(target.files)
    }, { id: `selection-${target.kind}`, providers: [target.kind] })
  }
  const local = plan.targets.find(target => target.kind === 'local')
  await record('local: owned virtualized selection', async (result = {}) => {
    result.phase = 'setup'
    const base = child(local.files, 'selection-virtual'), dest = child(local.files, 'selection-virtual-all')
    const paths = Array.from({ length: 200 }, (_, i) => child(base, `item-${String(i).padStart(3, '0')}.txt`))
    const bytes = new Map(paths.map((path, i) => [path, `Generated virtual row ${i}\n`]))
    await fixture.mkdir(base); await fixture.mkdir(dest)
    for (const [path, value] of bytes) await fixture.write(path, value)
    const step = stepper(result)
    await step('virtual-first-window-single', async () => {
      await ui.navigate(base); await ui.setView('list'); await ui.sort('Name', 'asc')
      await ui.virtualWindow(base, paths, paths[0]); await ui.select(paths[0]); await ui.selection(base, [paths[0]])
    })
    await step('virtual-arrow-to-offscreen-end', async () => {
      await ui.arrowSteps(paths.length - 1); await ui.virtualWindow(base, paths, paths.at(-1))
      await ui.selection(base, [paths.at(-1)])
    })
    await step('virtual-shift-offscreen-range', async () => {
      await ui.modifiedSelect(paths.at(-3), 'Shift'); await ui.selection(base, paths.slice(-3))
      await ui.selectionKey('ArrowUp', 'Shift'); await ui.selection(base, paths.slice(-4))
    })
    await step('virtual-ctrl-toggle-and-escape', async () => {
      await ui.modifiedSelect(paths.at(-2), 'Control'); await ui.selection(base, [paths.at(-4), paths.at(-3), paths.at(-1)])
      await ui.selectionKey('Escape'); await ui.selection(base, [])
    })
    await step('virtual-select-all-exact-copy', async part => {
      await ui.chord('a'); await ui.selection(base, paths)
      await ui.copySelection(dest, paths)
      part.ui = 'ACKNOWLEDGED'
      result.phase = 'verification'; await verifySelectedCopy(fixture, dest, paths, bytes)
      for (const [path, value] of bytes) assert.equal(await fixture.read(path), value)
    })
    result.phase = 'ui'; await ui.navigate(local.files)
  }, { id: 'selection-virtual-local', providers: ['local'] })
}

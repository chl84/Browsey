import { child } from './scope.mjs'
import { verifyByteTree } from './byte-tree.mjs'
import { releasedResources } from './resources.mjs'
import { waitActivityGone } from './progress.mjs'
import { recordPart } from './report.mjs'

const at = (base, name) => name.split('/').reduce(child, base)
export function treesManifest(plan) {
  return [...plan.targets.map(target => ({ id: `trees-${target.kind}`, name: `${target.kind}: bounded empty deep wide trees`,
    providers: [target.kind], partIds: ['reduced-tree-copy', 'deep-wide-copy', 'recursive-search-response'] })),
  { id: 'trees-virtual-local', name: 'local: 200-entry bounded native list', providers: ['local'],
    partIds: ['first-last-selection', 'select-all-copy-digests', 'return-small-cleanup'] }]
}
async function resources(ui, runId) {
  await waitActivityGone(ui)
  await releasedResources(ui, runId)
}
export async function trees(plan, fixture, ui, record) {
  ui.waitTimeout = 180_000; ui.transferTimeout = 600_000
  for (const target of plan.targets) await record(`${target.kind}: bounded empty deep wide trees`, async result => {
    result.phase = 'setup'
    const base = child(target.files, 'trees'), source = child(base, 'source'), destination = child(base, 'destination')
    for (const raw of [base, source, destination]) await fixture.mkdir(raw)
    const expected = new Map([['small', null], ['small/deep', null], ['small/deep/empty', null]])
    for (let index = 0; index < 3; index++) expected.set(`small/needle-small-${index}.bin`, Buffer.alloc(index + 1, index))
    const targetExpected = new Map()
    for (const [name, bytes] of expected) {
      if (bytes === null) await fixture.mkdir(at(source, name)); else await fixture.write(at(source, name), bytes)
    }
    const step = (id, action) => recordPart(result, id, async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; const start = Date.now(); await action(part)
      part.ui = 'ACKNOWLEDGED'; part.elapsedMs = Date.now() - start; result.phase = 'verification'
      part.source = await verifyByteTree(fixture, source, expected)
      part.destination = await verifyByteTree(fixture, destination, targetExpected)
      await resources(ui, plan.runId)
    })
    await step('reduced-tree-copy', async () => {
      await ui.setView('list'); await ui.populateClipboard(source, [child(source, 'small')], false, false, { directories: [child(source, 'small')] })
      await ui.paste(destination, child(destination, 'small'))
      for (const [name, bytes] of expected) targetExpected.set(name, bytes)
    })
    result.phase = 'setup'
    expected.set('tree', null)
    let deep = 'tree'
    for (let depth = 1; depth <= 6; depth++) { deep += `/d${depth}`; expected.set(deep, null) }
    expected.set(`${deep}/empty`, null); expected.set(`${deep}/needle-deep.bin`, Buffer.from([0, 255, 128]))
    for (let index = 0; index < 8; index++) expected.set(`tree/needle-wide-${index}.bin`, Buffer.alloc(index + 1, index + 128))
    expected.set('tree/empty-a', null); expected.set('tree/empty-b', null)
    for (const [name, bytes] of expected) if (name === 'tree' || name.startsWith('tree/')) {
      if (bytes === null) await fixture.mkdir(at(source, name)); else await fixture.write(at(source, name), bytes)
    }
    await step('deep-wide-copy', async () => {
      await ui.setView('grid'); await ui.populateClipboard(source, [child(source, 'tree')], false, true, { directories: [child(source, 'tree')] })
      await ui.paste(destination, child(destination, 'tree'), { menu: true })
      for (const [name, bytes] of expected) targetExpected.set(name, bytes)
    })
    await step('recursive-search-response', async () => {
      await ui.navigate(source); await ui.setView('grid'); await ui.search(source, 'needle', true)
      const matches = [...expected].filter(([, bytes]) => bytes !== null).map(([name]) => at(source, name))
      await ui.listing(source, 'grid', matches); await resources(ui, plan.runId); await ui.exitQuery(source)
      await ui.setView('list'); await ui.listing(source, 'list', [child(source, 'small'), child(source, 'tree')]); await ui.refresh()
    })
  }, { id: `trees-${target.kind}`, providers: [target.kind] })
  const local = plan.targets.find(target => target.kind === 'local')
  await record('local: 200-entry bounded native list', async result => {
    result.phase = 'setup'
    const base = child(local.files, 'trees-large'), target = child(local.files, 'trees-large-copy')
    await fixture.mkdir(base); await fixture.mkdir(target)
    const expected = new Map(Array.from({ length: 200 }, (_, index) => [`item-${String(index).padStart(3, '0')}.bin`, Buffer.from([index])]))
    for (const [name, bytes] of expected) await fixture.write(child(base, name), bytes)
    const paths = [...expected.keys()].map(name => child(base, name))
    await recordPart(result, 'first-last-selection', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; const start = Date.now()
      await ui.navigate(base); await ui.setView('list'); await ui.sort('Name', 'asc')
      await ui.virtualWindow(base, paths, paths[0]); await ui.select(paths[0]); await ui.selection(base, [paths[0]])
      await ui.arrowSteps(199); await ui.virtualWindow(base, paths, paths.at(-1)); await ui.selection(base, [paths.at(-1)])
      part.ui = 'ACKNOWLEDGED'; part.elapsedMs = Date.now() - start
    })
    await recordPart(result, 'select-all-copy-digests', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; await ui.chord('a'); await ui.selection(base, paths); await ui.copySelection(target, paths)
      part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'
      part.source = await verifyByteTree(fixture, base, expected, { maxEntries: 256 })
      part.destination = await verifyByteTree(fixture, target, expected, { maxEntries: 256 })
    })
    await recordPart(result, 'return-small-cleanup', async part => {
      const smallSource = child(child(local.files, 'trees'), 'source')
      result.phase = 'ui'; part.ui = 'STARTED'; await ui.navigate(smallSource)
      await ui.listing(smallSource, 'list', [child(smallSource, 'small'), child(smallSource, 'tree')])
      part.ui = 'ACKNOWLEDGED'; await resources(ui, plan.runId)
    })
  }, { id: 'trees-virtual-local', providers: ['local'] })
}

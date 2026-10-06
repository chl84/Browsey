import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'

export const transferPartIds = ['file', 'empty-folder', 'nested-tree', 'mixed-batch']
const sourceEntries = [
  ['single.txt', 'Generated single-file transfer bytes.\n'], ['empty', null],
  ['tree', null], ['tree/deep', null], ['tree/deep/empty', null],
  ['tree/deep/nested.txt', 'Generated nested transfer bytes.\n'],
  ['mixed.txt', 'Generated mixed-file transfer bytes.\n'], ['mixed-empty', null],
  ['mixed-tree', null], ['mixed-tree/nested.txt', 'Generated mixed nested bytes.\n'],
  ['unrelated-source.txt', 'Preserve unrelated source exactly.\n'],
]
const targetEntries = [['unrelated-target.txt', 'Preserve unrelated destination exactly.\n']]
const at = (base, relative) => relative.split('/').reduce((parent, name) => child(parent, name), base)

export function transferRoutes(plan, group = 'within') {
  assert.equal(group, 'within', 'Expected a supported transfer group')
  return plan.targets.map(target => ({ from: target.kind, to: target.kind }))
}

export function transferManifest(plan, group = 'within') {
  return transferRoutes(plan, group).flatMap(({ from, to }) => ['copy', 'move'].map(operation => ({
    id: `${operation}-rich-${from}-${to}`, name: `${operation}: ${from} to ${to}, exact trees`,
    providers: [...new Set([from, to])], partIds: transferPartIds,
  })))
}

async function seed(fixture, base, entries) {
  await fixture.mkdir(base)
  for (const [name, bytes] of entries) {
    if (bytes === null) await fixture.mkdir(at(base, name))
    else await fixture.write(at(base, name), bytes)
  }
  return new Map(entries)
}

async function transferCase(source, target, operation, fixture, ui, result) {
  result.phase = 'setup'
  const label = `${operation}-rich-${source.kind}-${target.kind}`
  const from = child(source.files, `${label}-source`), to = child(target.files, `${label}-target`)
  const expectedSource = await seed(fixture, from, sourceEntries)
  const expectedTarget = await seed(fixture, to, targetEntries)
  const move = operation === 'move'
  await ui.setView(move ? 'grid' : 'list')
  for (const [id, names] of [
    ['file', ['single.txt']], ['empty-folder', ['empty']], ['nested-tree', ['tree']],
    ['mixed-batch', ['mixed.txt', 'mixed-empty', 'mixed-tree']],
  ]) await recordPart(result, id, async part => {
    result.phase = 'ui'; part.ui = 'STARTED'
    const paths = names.map(name => at(from, name))
    const directories = names.filter(name => expectedSource.get(name) === null).map(name => at(from, name))
    const menu = id === 'empty-folder' || id === 'mixed-batch'
    await ui.populateClipboard(from, paths, move, menu, { directories })
    await ui.paste(to, at(to, names[0]), { menu })
    part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'
    for (const [relative, bytes] of [...expectedSource]) {
      if (!names.some(name => relative === name || relative.startsWith(`${name}/`))) continue
      expectedTarget.set(relative, bytes)
      if (move) expectedSource.delete(relative)
    }
    // Verify both whole sides, including empty directories and unrelated bytes.
    // A destination success cannot conceal premature source removal or extras.
    await verifyTree(fixture, from, expectedSource)
    await verifyTree(fixture, to, expectedTarget)
  })
}

export async function transfers(plan, fixture, ui, record, group = 'within') {
  for (const item of transferManifest(plan, group)) {
    const [operation, , from, to] = item.id.split('-')
    const source = plan.targets.find(target => target.kind === from)
    const target = plan.targets.find(target => target.kind === to)
    await record(item.name, result => transferCase(source, target, operation, fixture, ui, result),
      { id: item.id, providers: item.providers })
  }
}

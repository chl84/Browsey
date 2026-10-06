import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'

const choices = { skip: 'Skip', rename: 'Auto-rename', cancel: 'Cancel', overwrite: 'Overwrite' }
const conflictRoutes = plan => [...plan.targets.map(target => ({ from: target.kind, to: target.kind })),
  ...(plan.targets.some(t => t.kind === 'cloud') ? [{ from: 'local', to: 'cloud' }, { from: 'cloud', to: 'local' }] : [])]
export const conflictManifest = plan => {
  const cases = conflictRoutes(plan).flatMap(({ from, to }) => ['copy', 'move'].flatMap(operation =>
    Object.keys(choices).map(policy => ({ id: `${operation}-conflicts-${from}-${to}-${policy}`,
      name: `${operation} conflicts: ${from} to ${to}, ${policy}`, providers: [...new Set([from, to])],
      partIds: operation === 'copy' ? ['same-kind', 'file-over-directory', 'directory-over-file'] : ['same-kind'] }))))
  const priority = item => item.id.endsWith('-overwrite') && item.providers.includes('cloud') ? 0
    : item.id.endsWith('-overwrite') && item.providers.includes('mobile') ? 1 : 2
  return cases.sort((a, b) => priority(a) - priority(b))
}

const at = (base, relative) => relative.split('/').reduce((parent, name) => child(parent, name), base)
const renamed = name => { const dot = name.lastIndexOf('.'); return dot > 0 ? `${name.slice(0, dot)}-2${name.slice(dot)}` : `${name}-2` }
async function seed(fixture, base, entries, label) {
  await fixture.mkdir(base)
  const expected = new Map()
  for (const [name, content] of entries) {
    const bytes = content === null ? null : `${label}/${name}\n${content}`
    if (bytes === null) await fixture.mkdir(at(base, name)); else await fixture.write(at(base, name), bytes)
    expected.set(name, bytes)
  }
  return expected
}

export async function conflicts(plan, fixture, ui, record) {
  for (const item of conflictManifest(plan)) await record(item.name, async result => {
    const [operation, , sourceKind, targetKind, policy] = item.id.split('-')
    const sourceTarget = plan.targets.find(t => t.kind === sourceKind), target = plan.targets.find(t => t.kind === targetKind)
    const move = operation === 'move'
    await ui.setView(move ? 'grid' : 'list')
    for (const id of item.partIds) await recordPart(result, id, async part => {
      result.phase = 'setup'
      const label = `${item.id}-${id}`, from = child(sourceTarget.files, `${label}-source`), to = child(target.files, `${label}-target`)
      const same = id === 'same-kind', file = id === 'file-over-directory'
      const source = same ? [['file.txt', 'new file'], ['folder', null], ['folder/nested.txt', 'new nested'],
        ['folder/empty', null], ['fresh.txt', 'new nonconflicting']] : file ? [['item', 'new file']]
        : [['item', null], ['item/nested.txt', 'new nested'], ['item/empty', null]]
      const destination = same ? [['file.txt', 'old file'], ['file-1.txt', 'reserved file'], ['folder', null],
        ['folder/nested.txt', 'old nested'], ['folder/target-only.txt', 'old unrelated nested'], ['folder-1', null]]
        : file ? [['item', null], ['item/nested.txt', 'old nested'], ['item/empty', null]] : [['item', 'old file']]
      if (!same) destination.push(['item-1', 'reserved name'])
      source.push(['unrelated-source.txt', 'preserve source']); destination.push(['unrelated-target.txt', 'preserve destination'])
      const expectedSource = await seed(fixture, from, source, `${label}-source`)
      const expectedTarget = await seed(fixture, to, destination, `${label}-target`)
      const names = same ? ['file.txt', 'folder', 'fresh.txt'] : ['item']
      result.phase = 'ui'; part.ui = 'STARTED'
      await ui.populateClipboard(from, names.map(name => at(from, name)), move, true,
        { directories: names.filter(name => expectedSource.get(name) === null).map(name => at(from, name)) })
      const refusal = !same && policy === 'overwrite' && (sourceKind === 'cloud' || targetKind === 'cloud')
      const first = same && policy === 'skip' ? 'fresh.txt' : policy === 'rename' ? renamed(names[0]) : names[0]
      await ui.paste(to, at(to, first), { menu: true, conflict: choices[policy],
        ...(refusal ? { expectedError: /Cannot overwrite a file with a folder or a folder with a file/i } : {}) })
      part.ui = 'ACKNOWLEDGED'; part.outcome = refusal ? 'EXPLICIT_REFUSAL' : policy === 'cancel' ? 'CANCELLED' : 'COMPLETED'
      result.phase = 'verification'
      if (!refusal && policy !== 'cancel') for (const name of names) {
        if (policy === 'skip' && expectedTarget.has(name)) continue
        const finalName = policy === 'rename' && expectedTarget.has(name) ? renamed(name) : name
        if (policy === 'overwrite' && !same) for (const key of [...expectedTarget.keys()]) {
          if (key === name || key.startsWith(`${name}/`)) expectedTarget.delete(key)
        }
        for (const [relative, bytes] of [...expectedSource]) if (relative === name || relative.startsWith(`${name}/`)) {
          expectedTarget.set(`${finalName}${relative.slice(name.length)}`, bytes)
          if (move) expectedSource.delete(relative)
        }
      }
      await verifyTree(fixture, from, expectedSource); await verifyTree(fixture, to, expectedTarget)
      assert.ok(expectedSource.has('unrelated-source.txt') && expectedTarget.has('unrelated-target.txt'))
    })
  }, { id: item.id, providers: item.providers })
}

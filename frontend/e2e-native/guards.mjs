import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'
export const guardManifest = (plan, group = 'all') => plan.targets.filter(target => group === 'all' || ['cloud', 'mobile'].includes(target.kind)).map(target => ({ id: `guards-${target.kind}`,
  name: `Reject unsafe transfers: ${target.kind}`, providers: [target.kind],
  partIds: ['cut-same-file', 'cut-same-directory', 'copy-descendant', 'move-descendant', 'copy-into-self', 'move-into-self', 'copy-ancestor', 'move-ancestor', 'same-folder-copy-new-name',
    ...(target.kind === 'cloud' ? ['copy-casefold-descendant', 'move-casefold-descendant'] : [])].filter(id => group === 'all' || target.kind === 'mobile' || id.includes('casefold')) }))
const at = (base, relative) => relative.split('/').reduce((parent, name) => child(parent, name), base)
const entries = [['single.txt', 'Generated same-file bytes'], ['Folder', null], ['Folder/sub', null],
  ['Folder/empty', null], ['Folder/nested.txt', 'Generated nested bytes'], ['unrelated.txt', 'Preserve unrelated bytes']]
export async function guards(plan, fixture, ui, record, group = 'all') {
  for (const item of guardManifest(plan, group)) await record(item.name, async result => {
    const target = plan.targets.find(t => t.kind === item.providers[0])
    await ui.setView('list')
    for (const id of item.partIds) await recordPart(result, id, async part => {
      result.phase = 'setup'
      const base = child(target.files, `${item.id}-${id}`), expected = new Map()
      await fixture.mkdir(base)
      const ancestor = id.endsWith('-ancestor')
      for (const [name, content] of [...entries, ...(ancestor ? [['Folder/Folder', null], ['Folder/Folder/preserve.txt', 'Preserve ancestor-source bytes'], ['Folder/Folder/empty', null]] : [])]) {
        const bytes = content === null ? null : `${item.id}/${id}/${name}\n${content}`
        if (bytes === null) await fixture.mkdir(at(base, name)); else await fixture.write(at(base, name), bytes)
        expected.set(name, bytes)
      }
      const unique = id === 'same-folder-copy-new-name', file = id === 'cut-same-file' || unique
      const source = at(base, ancestor ? 'Folder/Folder' : file ? 'single.txt' : 'Folder'), cut = id.startsWith('cut') || id.startsWith('move')
      const descendant = id.includes('descendant'), alias = id.includes('casefold')
      const destination = id.startsWith('cut') || unique || ancestor ? base : at(base, descendant ? 'Folder/sub' : 'Folder')
      result.phase = 'ui'; part.ui = 'STARTED'
      await ui.populateClipboard(ancestor ? at(base, 'Folder') : base, [source], cut, true, { directories: file ? [] : [source] })
      let actualDestination = destination
      if (alias) {
        actualDestination = destination.replace('/Folder/', '/folder/')
        await ui.navigate(destination)
        await ui.enterPath(actualDestination)
      }
      const started = Date.now()
      await ui.paste(actualDestination, unique ? at(base, 'single-1.txt') : at(destination, file ? 'single.txt' : 'Folder'),
        { menu: true, ...(ancestor ? { conflict: 'Overwrite' } : {}), ...(alias ? { alreadyAt: true } : {}), ...(!unique ? {
          expectedError: /Source and destination are the same|Cannot paste a directory into itself|Cannot transfer a folder into itself or its descendant|Cannot overwrite a parent directory of the source item/i,
        } : {}) })
      part.ui = 'ACKNOWLEDGED'; part.outcome = unique ? 'DISTINCT_UNIQUE_TARGET' : 'EXPLICIT_REJECTION'
      part.elapsedMs = Date.now() - started
      assert.ok(part.elapsedMs < 180_000, 'Unsafe transfers must reject within a bounded observation')
      result.phase = 'verification'
      if (unique) expected.set('single-1.txt', expected.get('single.txt'))
      await verifyTree(fixture, base, expected)
    })
  }, { id: item.id, providers: item.providers })
}

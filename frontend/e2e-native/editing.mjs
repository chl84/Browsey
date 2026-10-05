import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { child } from './scope.mjs'
import { recordPart } from './report.mjs'

const leaf = (base, relative) => relative.split('/').reduce((parent, name) => child(parent, name), base)

const sentinel = 'Generated edit fixture: preserve exactly.\n'
export const renameParts = ['file-extension', 'nonempty-folder', 'file-collision', 'folder-collision',
  'file-versus-folder', 'folder-versus-file', 'cancel-file', 'escape-folder', 'file-case-only',
  'folder-case-only', 'repeat-enter', 'correct-rejection']
export const fileopsParts = ['no-selection', 'mixed-copy-keyboard-repeat', 'mixed-cut-menu',
  'mixed-copy-menu', 'mixed-cut-keyboard-repeat', 'cancel-delete-button', 'cancel-delete-escape', 'mixed-delete-menu-repeat']
export const propertiesParts = ['file-keyboard', 'folder-menu', 'mixed-keyboard']

export const propertiesSizeMatches = (actual, expected) => typeof actual === 'string'
  && actual.replace(/\s+/g, '') === expected.replace(/\s+/g, '')

export function editingManifest(plan, group = 'editing') {
  const cases = []
  for (const target of plan.targets) for (const operation of ['fileops', 'rename', 'properties']) {
    if (group !== 'editing' && group !== operation) continue
    const prefix = operation === 'rename' ? 'rename-edge' : operation
    cases.push({ id: `${prefix}-${target.kind}`, name: `${target.kind}: ${operation} edge cases`, providers: [target.kind] })
  }
  if (group === 'editing' || group === 'history') cases.push({ id: 'history-local', name: 'local: history boundaries and restart', providers: ['local'] })
  return cases
}

// Read actual generated trees, not an IPC receipt or the displayed list. Every
// read stays inside this case's owned folder; no provider root inventory.
export async function verifyTree(fixture, base, expected) {
  // At most two independent reads. Await every started child even after one
  // fails; neither verification nor teardown may outlive a fixture subprocess.
  const reads = async actions => {
    const results = await Promise.allSettled(actions.map(action => Promise.resolve().then(action)))
    for (const result of results) if (result.status === 'rejected') throw result.reason
    return results.map(result => result.value)
  }
  const enumerate = async () => {
    const actual = new Map(), files = [], directories = [{ path: base, depth: 0 }]
    while (directories.length) {
      const group = directories.splice(0, 2)
      for (const directory of group) assert.ok(directory.depth <= 4, 'Generated tree depth bound')
      const snapshots = await reads(group.map(directory => () => fixture.snapshot(directory.path, { maxChildren: 128 })))
      for (const [index, entries] of snapshots.entries()) for (const entry of entries) {
        assert.ok(entry.path.startsWith(`${base}/`), 'Observed entry must remain below this owned case')
        assert.ok(['file', 'dir'].includes(entry.kind), 'Generated entry kind')
        const relative = entry.path.slice(base.length + 1)
        assert.ok(!actual.has(relative) && actual.size < 128, 'Generated entry bound')
        actual.set(relative, entry.kind === 'dir' ? null : undefined)
        if (entry.kind === 'dir' && expected.has(relative) && expected.get(relative) === null) directories.push({ path: entry.path, depth: group[index].depth + 1 })
        else if (entry.kind === 'file') files.push({ path: entry.path, relative })
      }
    }
    return { actual, files }
  }
  const expectedShape = [...expected].map(([path, bytes]) => [path, bytes === null ? 'dir' : 'file']).sort()
  let actual, files
  for (let attempt = 0; ; attempt++) {
    ({ actual, files } = await enumerate())
    const shape = [...actual].map(([path, bytes]) => [path, bytes === null ? 'dir' : 'file']).sort()
    if (JSON.stringify(shape) === JSON.stringify(expectedShape)) break
    // Remote listings can briefly retain just-deleted entries. Only metadata
    // readiness is polled (four bounded snapshots); mutations and bytes are
    // never retried, and unexpected directories are not descended into.
    if (!base.startsWith('rclone://') || attempt === 3) assert.deepEqual(shape, expectedShape, 'Exact generated paths/kinds must match before content reads')
    await new Promise(resolve => setTimeout(resolve, 1000))
  }
  for (let index = 0; index < files.length; index += 2) {
    const group = files.slice(index, index + 2)
    const contents = await reads(group.map(file => () => fixture.read(file.path)))
    for (const [index, file] of group.entries()) actual.set(file.relative, contents[index])
  }
  assert.deepEqual([...actual].sort(), [...expected].sort(), 'Exact generated paths/kinds/bytes must match; no duplicate or damaged entries')
}

async function seed(fixture, base, entries) {
  await fixture.mkdir(base)
  for (const [name, value] of entries) {
    if (value === null) await fixture.mkdir(leaf(base, name))
    else await fixture.write(leaf(base, name), value)
  }
  return new Map(entries)
}
function relocate(expected, from, to) {
  for (const [name, value] of [...expected]) if (name === from || name.startsWith(`${from}/`)) {
    expected.delete(name); expected.set(`${to}${name.slice(from.length)}`, value)
  }
}
function copyTree(expected, from, to) {
  for (const [name, value] of [...expected]) if (name === from || name.startsWith(`${from}/`)) expected.set(`${to}${name.slice(from.length)}`, value)
}
function eraseTree(expected, from) {
  for (const name of [...expected.keys()]) if (name === from || name.startsWith(`${from}/`)) expected.delete(name)
}
function steps(result, fixture, base, expected) {
  return (id, action) => recordPart(result, id, async part => {
    result.phase = 'ui'; part.ui = 'STARTED'
    await action(); part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'
    await verifyTree(fixture, base, expected)
  })
}

async function renameCase(target, fixture, ui, result) {
  result.phase = 'setup'
  const base = leaf(target.files, 'rename-edges')
  const expected = await seed(fixture, base, [['original.txt', sentinel], ['tree', null],
    ['tree/nested.txt', sentinel], ['occupied.txt', 'Existing file stays.\n'], ['occupied-folder', null],
    ['occupied-folder/preserved.txt', 'Existing folder stays.\n'], ['unrelated.txt', sentinel]])
  await ui.navigate(base); await ui.setView('list')
  const step = steps(result, fixture, base, expected)
  const change = async (oldName, name, options = {}) => {
    await ui.beginRename(leaf(base, oldName), options.menu)
    await ui.renameDraft(name, options); relocate(expected, oldName, name)
    await ui.editingFocus(base, [leaf(base, name)])
  }
  await step('file-extension', () => change('original.txt', 'renamed.data'))
  await step('nonempty-folder', () => change('tree', 'renamed-tree', { menu: true, button: true }))
  for (const [id, source, occupied] of [['file-collision', 'renamed.data', 'occupied.txt'],
    ['folder-collision', 'renamed-tree', 'occupied-folder'], ['file-versus-folder', 'renamed.data', 'occupied-folder'],
    ['folder-versus-file', 'renamed-tree', 'occupied.txt']]) {
    await step(id, async () => {
      await ui.beginRename(leaf(base, source)); await ui.renameDraft(occupied, { error: /already exists|destination exists/i })
      result.phase = 'verification'; await verifyTree(fixture, base, expected)
      result.phase = 'ui'; await ui.cancelRename(); await ui.editingFocus(base, [leaf(base, source)])
    })
  }
  await step('cancel-file', async () => { await ui.beginRename(leaf(base, 'renamed.data')); await ui.fill(await ui.browser.$('#rename-entry-name'), 'cancelled.txt'); await ui.cancelRename(); await ui.editingFocus(base, [leaf(base, 'renamed.data')]) })
  await step('escape-folder', async () => { await ui.beginRename(leaf(base, 'renamed-tree')); await ui.cancelRename(true); await ui.editingFocus(base, [leaf(base, 'renamed-tree')]) })
  await ui.setView('grid')
  await step('file-case-only', () => change('renamed.data', 'RENAMED.data'))
  await step('folder-case-only', () => change('renamed-tree', 'RENAMED-TREE', { menu: true }))
  await step('repeat-enter', () => change('RENAMED.data', 'once.txt', { repeat: true }))
  await step('correct-rejection', async () => {
    await ui.beginRename(leaf(base, 'once.txt')); await ui.renameDraft('occupied.txt', { error: /already exists|destination exists/i })
    result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.renameDraft('corrected.txt'); relocate(expected, 'once.txt', 'corrected.txt'); await ui.editingFocus(base, [leaf(base, 'corrected.txt')])
  })
}

async function fileopsCase(target, fixture, ui, result) {
  result.phase = 'setup'
  const base = leaf(target.files, 'fileops-edges')
  const expected = await seed(fixture, base, [['source', null], ['source/a.txt', sentinel], ['source/tree', null],
    ['source/tree/nested.txt', sentinel], ['source/unrelated.txt', 'Unrelated source stays.\n'],
    ['copy-key', null], ['cut-menu', null], ['copy-menu', null], ['cut-key', null]])
  const source = leaf(base, 'source'), paths = [leaf(source, 'a.txt'), leaf(source, 'tree')]
  await ui.navigate(source); await ui.setView('list')
  const step = steps(result, fixture, base, expected)
  await step('no-selection', async () => {
    await ui.emptySpace(source)
    await ui.chord('c'); await ui.expectedToast(/^Copy failed: Nothing selected$/)
    await ui.chord('x'); await ui.expectedToast(/^Cut failed: Nothing selected$/)
    await ui.chord('Delete', 'Shift'); await ui.idle(); await ui.selection(source, [])
    // No-selection paste is a clipboard operation. On the first local case the
    // isolated app has an empty clipboard; never import a desktop clipboard.
    if (target.kind === 'local') { await ui.chord('v'); await ui.idle({ toast: 'Clipboard is empty' }) }
  })
  await step('mixed-copy-keyboard-repeat', async () => {
    await ui.populateClipboard(source, paths, false, false, { directories: [paths[1]] }); await ui.paste(leaf(base, 'copy-key'), leaf(base, 'copy-key/a.txt'), { repeat: true })
    for (const name of ['a.txt', 'tree']) copyTree(expected, `source/${name}`, `copy-key/${name}`)
  })
  await step('mixed-cut-menu', async () => {
    await ui.populateClipboard(source, paths, true, true, { directories: [paths[1]] }); await ui.paste(leaf(base, 'cut-menu'), leaf(base, 'cut-menu/a.txt'), { menu: true })
    for (const name of ['a.txt', 'tree']) relocate(expected, `source/${name}`, `cut-menu/${name}`)
  })
  const moved = leaf(base, 'cut-menu'), movedPaths = [leaf(moved, 'a.txt'), leaf(moved, 'tree')]
  await ui.setView('grid')
  await step('mixed-copy-menu', async () => {
    await ui.populateClipboard(moved, movedPaths, false, true, { directories: [movedPaths[1]] }); await ui.paste(leaf(base, 'copy-menu'), leaf(base, 'copy-menu/a.txt'), { menu: true })
    for (const name of ['a.txt', 'tree']) copyTree(expected, `cut-menu/${name}`, `copy-menu/${name}`)
  })
  await step('mixed-cut-keyboard-repeat', async () => {
    await ui.populateClipboard(moved, movedPaths, true, false, { directories: [movedPaths[1]] }); await ui.paste(leaf(base, 'cut-key'), leaf(base, 'cut-key/a.txt'), { repeat: true })
    for (const name of ['a.txt', 'tree']) relocate(expected, `cut-menu/${name}`, `cut-key/${name}`)
  })
  const deleted = leaf(base, 'cut-key'), deletePaths = [leaf(deleted, 'a.txt'), leaf(deleted, 'tree')]
  for (const [id, escape] of [['cancel-delete-button', false], ['cancel-delete-escape', true]]) await step(id, async () => {
    await ui.navigate(deleted); await ui.select(deletePaths[0]); await ui.modifiedSelect(deletePaths[1], 'Control'); await ui.selection(deleted, deletePaths, { directories: [deletePaths[1]] })
    await ui.deleteSelection({ cancel: true, escape }); await ui.editingFocus(deleted, deletePaths)
  })
  await step('mixed-delete-menu-repeat', async () => {
    await ui.navigate(deleted); await ui.select(deletePaths[0]); await ui.modifiedSelect(deletePaths[1], 'Control'); await ui.selection(deleted, deletePaths, { directories: [deletePaths[1]] })
    await ui.deleteSelection({ menu: true, raw: deletePaths[0], repeat: true })
    for (const name of ['a.txt', 'tree']) eraseTree(expected, `cut-key/${name}`)
  })
}

async function propertiesCase(target, fixture, ui, result) {
  result.phase = 'setup'
  const base = leaf(target.files, 'properties-edges'), file = leaf(base, 'sample.txt'), folder = leaf(base, 'tree')
  const expected = await seed(fixture, base, [['sample.txt', sentinel], ['tree', null], ['tree/nested.txt', sentinel], ['unrelated.txt', sentinel]])
  await ui.navigate(base); await ui.setView('list')
  const step = steps(result, fixture, base, expected)
  for (const [id, paths, bytes, kind, menu] of [['file-keyboard', [file], Buffer.byteLength(sentinel), 'file', false],
    ['folder-menu', [folder], Buffer.byteLength(sentinel), 'dir', true], ['mixed-keyboard', [file, folder], Buffer.byteLength(sentinel) * 2, null, false]]) {
    await step(id, async () => {
      await ui.propertiesOpen(base, paths, menu, { directories: paths.filter(path => path === folder) })
      const measured = target.kind !== 'cloud' || kind === 'file'
      const items = paths.reduce((count, path) => count + (path === folder ? 2 : 1), 0)
      const size = measured ? `${bytes} B (${items} ${items === 1 ? 'item' : 'items'})` : '—'
      let observedSize
      try {
        await ui.browser.waitUntil(async () => {
          observedSize = (await ui.propertiesRows()).Size
          return propertiesSizeMatches(observedSize, size)
        }, { timeout: 60_000, timeoutMsg: 'Properties must show independently known generated size and item count' })
      } catch (error) { throw new Error(`${error.message}; expected ${JSON.stringify(size)}, observed ${JSON.stringify(observedSize)}`, { cause: error }) }
      const rows = await ui.propertiesRows()
      if (kind) { assert.equal(rows.Name, paths[0].split('/').at(-1)); assert.equal(rows.Type, kind); assert.equal(rows['Parent folder'], 'properties-edges') }
      else { assert.ok(!rows.Name && !rows.Type) }
      await ui.propertiesTab('Extra')
      const extra = await (await ui.browser.$('.properties-modal')).getText()
      assert.ok(!/Failed to load/.test(extra), 'Extra metadata must finish successfully or report unsupported metadata')
      if (paths.length === 2) assert.match(extra, /one item|single item|multiple/i)
      await ui.propertiesTab('Ownership')
      const ownership = await (await ui.browser.$('.properties-modal')).getText()
      const editable = await (await ui.browser.$('.properties-modal .ownership-apply-button')).isExisting()
      if (target.kind === 'local') assert.equal(editable, true, 'Owned local items must expose ownership capability')
      if (target.kind === 'usb') {
        if (editable) assert.equal(await (await ui.browser.$('.properties-modal .ownership-apply-button')).isEnabled(), true)
        else assert.match(ownership, /mount options|read.only|not supported/i, 'Managed USB ownership must explain its restriction')
      }
      if (target.kind === 'cloud') { assert.equal(editable, false); assert.match(ownership, /not supported/i) }
      if (['network', 'mobile'].includes(target.kind)) { assert.equal(editable, false); assert.match(ownership, /server|mount|not supported/i) }
      await ui.propertiesTab('Permissions')
      if (target.kind === 'cloud') assert.match(await (await ui.browser.$('.properties-modal')).getText(), /Not available/i)
      else {
        const metadata = await Promise.all(paths.map(path => fs.lstat(path)))
        for (const [scope, shift] of [['Owner', 6], ['Group', 3], ['Other users', 0]]) for (const [label, mask] of [['read', 4], ['write', 2], ['execute', 1]]) {
          const input = await ui.browser.$(`.properties-modal input[aria-label="${scope} ${label} permission"]`)
          const bits = metadata.map(stat => Boolean((stat.mode >> shift) & mask))
          assert.equal(await input.isSelected(), bits.every(Boolean))
          assert.equal(await ui.browser.execute(node => node.indeterminate, input), bits.some(Boolean) && !bits.every(Boolean))
          if (['network', 'mobile'].includes(target.kind)) assert.equal(await input.isEnabled(), false)
        }
      }
      await ui.closeProperties(base)
      await ui.editingFocus(base, paths)
      if (id === 'folder-menu') await ui.setView('grid')
    })
  }
}

async function historyCase(target, fixture, ui, result) {
  result.phase = 'setup'
  const base = leaf(target.files, 'history-edges')
  const expected = await seed(fixture, base, [['source', null], ['source/move.txt', sentinel], ['source/rename.txt', sentinel],
    ['source/delete-tree', null], ['source/delete-tree/nested.txt', sentinel], ['source/overwrite.txt', 'New source bytes.\n'],
    ['dest', null], ['dest/overwrite.txt', 'Original destination bytes.\n'], ['unrelated.txt', sentinel]])
  await ui.navigate(base); await ui.setView('list')
  const step = steps(result, fixture, base, expected), source = leaf(base, 'source'), dest = leaf(base, 'dest')
  const cycle = async (name, action, forward, backward, before, after) => step(name, async () => {
    await action(); forward(); result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.historyStep(false, before); backward(); result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.historyStep(true, after); forward()
  })
  await cycle('move-undo-redo', () => ui.transfer(leaf(source, 'move.txt'), dest, true),
    () => relocate(expected, 'source/move.txt', 'dest/move.txt'), () => relocate(expected, 'dest/move.txt', 'source/move.txt'),
    { absentPath: leaf(dest, 'move.txt') }, { resultPath: leaf(dest, 'move.txt') })
  await cycle('rename-undo-redo', () => ui.rename(leaf(source, 'rename.txt'), 'renamed.txt'),
    () => relocate(expected, 'source/rename.txt', 'source/renamed.txt'), () => relocate(expected, 'source/renamed.txt', 'source/rename.txt'),
    { resultPath: leaf(source, 'rename.txt') }, { resultPath: leaf(source, 'renamed.txt') })
  await cycle('delete-undo-redo', () => ui.remove(leaf(source, 'delete-tree')),
    () => eraseTree(expected, 'source/delete-tree'), () => { expected.set('source/delete-tree', null); expected.set('source/delete-tree/nested.txt', sentinel) },
    { resultPath: leaf(source, 'delete-tree') }, { absentPath: leaf(source, 'delete-tree') })
  await cycle('overwrite-undo-redo', async () => {
    await ui.populateClipboard(source, [leaf(source, 'overwrite.txt')]); await ui.paste(dest, leaf(dest, 'overwrite.txt'), { conflict: 'Overwrite' })
  }, () => expected.set('dest/overwrite.txt', 'New source bytes.\n'), () => expected.set('dest/overwrite.txt', 'Original destination bytes.\n'), {}, {})
  await step('redo-invalidation', async () => {
    await ui.historyStep(false); expected.set('dest/overwrite.txt', 'Original destination bytes.\n')
    result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.navigate(base); await ui.beginCreation(base, false); await ui.creationValue(false, 'new-action.txt'); await ui.submitCreation(false)
    expected.set('new-action.txt', ''); await ui.historyStep(true, {}, true)
  })
  // 51 completed single renames replace all preceding history. Exactly 50 undo
  // steps leave the first rename applied; the 51st must be explicitly refused.
  let previous = 'unrelated.txt'
  for (let i = 1; i <= 51; i++) await step(`limit-record-${i}`, async () => {
    await ui.navigate(base); await ui.beginRename(leaf(base, previous)); const next = `limit-${i}.txt`
    await ui.renameDraft(next); relocate(expected, previous, next); previous = next
  })
  for (let i = 51; i >= 2; i--) await step(`limit-undo-${i}`, async () => {
    const next = `limit-${i - 1}.txt`; await ui.historyStep(false, { resultPath: leaf(base, next) }); relocate(expected, previous, next); previous = next
  })
  await step('limit-undo-refused', () => ui.historyStep(false, {}, true))
  await step('restart-history-cleared', async () => {
    // Populate both undo and redo before restart, so both unavailable checks
    // prove lost session history rather than an already empty redo stack.
    await ui.beginRename(leaf(base, previous)); await ui.renameDraft('restart-retained.txt'); relocate(expected, previous, 'restart-retained.txt')
    result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.beginRename(leaf(base, 'restart-retained.txt')); await ui.renameDraft('restart-undone.txt'); relocate(expected, 'restart-retained.txt', 'restart-undone.txt')
    result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.historyStep(false, { resultPath: leaf(base, 'restart-retained.txt') }); relocate(expected, 'restart-undone.txt', 'restart-retained.txt')
    result.phase = 'verification'; await verifyTree(fixture, base, expected); result.phase = 'ui'
    await ui.restart(); await ui.navigate(base); await ui.historyStep(false, {}, true); await ui.historyStep(true, {}, true)
  })
}

export async function editing(plan, fixture, ui, record, group = 'editing') {
  for (const target of plan.targets) for (const [operation, action] of [['fileops', fileopsCase], ['rename', renameCase], ['properties', propertiesCase]]) {
    if (group !== 'editing' && group !== operation) continue
    const id = operation === 'rename' ? `rename-edge-${target.kind}` : `${operation}-${target.kind}`
    await record(`${target.kind}: ${operation} edge cases`, result => action(target, fixture, ui, result), { id, providers: [target.kind] })
  }
  if (group === 'editing' || group === 'history') {
    const target = plan.targets.find(target => target.kind === 'local')
    await record('local: history boundaries and restart', result => historyCase(target, fixture, ui, result), { id: 'history-local', providers: ['local'] })
  }
}

import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'

export const specialNames = ['space name.txt', 'æøå.txt', 'emoji-🧪.txt', 'e\u0301-combining.txt',
  'quotes-\'".txt', 'hash#percent%26&ampersand&_under.txt', '.leading.txt', '-leading.txt']
const folderName = 'folder # % æ'
export function namesManifest(plan) {
  return plan.targets.map(target => ({ id: `names-${target.kind}`, name: `${target.kind}: exact special names`,
    providers: [target.kind], partIds: [...(target.kind === 'mobile' ? ['quoted-name-rejection'] : []), 'list-grid', 'encoded-folder-navigation', 'copy', 'move', 'rename-emoji',
      ...(target.kind === 'local' ? ['create-trailing-file', 'create-trailing-folder', 'rename-trailing'] : [])] }))
}

export async function names(plan, fixture, ui, record) {
  ui.waitTimeout = 180_000; ui.transferTimeout = 600_000
  for (const target of plan.targets) await record(`${target.kind}: exact special names`, async result => {
    result.phase = 'setup'
    const base = child(target.files, 'names'), source = child(base, 'source')
    const copied = child(base, 'copied'), moved = child(base, 'moved')
    for (const path of [base, source, copied, moved]) await fixture.mkdir(path)
    const validNames = specialNames.filter(name => target.kind !== 'mobile' || !name.includes('"'))
    const expected = new Map(validNames.map((name, index) => [name, `Generated ${target.kind} special name ${index}: ${name}\n`]))
    expected.set(folderName, null); expected.set(`${folderName}/sentinel.txt`, 'Nested special-folder sentinel.\n')
    for (const [name, bytes] of expected) {
      const raw = name.split('/').reduce(child, source)
      if (bytes === null) await fixture.mkdir(raw); else await fixture.write(raw, bytes)
    }
    const entries = [...validNames, folderName], paths = entries.map(name => child(source, name))
    const step = (id, action, verify) => recordPart(result, id, async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; await action(); part.ui = 'ACKNOWLEDGED'
      result.phase = 'verification'; await verify()
    })
    await ui.navigate(source); await ui.hidden(true)
    if (target.kind === 'mobile') await step('quoted-name-rejection', async () => {
      await ui.beginCreation(source, false); await ui.creationValue(false, specialNames.find(name => name.includes('"')))
      await ui.submitCreation(false, { error: /input\/output error|not supported|invalid.*name|EIO|not allowed/i })
      result.phase = 'verification'; await verifyTree(fixture, source, expected)
      await ui.cancelCreation(false)
    }, () => verifyTree(fixture, source, expected))
    await step('list-grid', async () => {
      for (const view of ['list', 'grid']) { await ui.setView(view); await ui.listing(source, view, paths) }
    }, () => verifyTree(fixture, source, expected))
    await step('encoded-folder-navigation', async () => {
      const folder = child(source, folderName)
      await ui.openFolder(folder); await ui.listing(folder, 'grid', [child(folder, 'sentinel.txt')])
      assert.equal(await (await ui.browser.$('#explorer-path-input')).getValue(), folder, 'Displayed path must preserve literal URI/name characters')
    }, () => verifyTree(fixture, source, expected))
    await step('copy', async () => {
      await ui.setView('list'); await ui.populateClipboard(source, paths, false, false, { directories: [child(source, folderName)] })
      await ui.paste(copied, child(copied, entries[0]))
    }, async () => { await verifyTree(fixture, source, expected); await verifyTree(fixture, copied, expected) })
    await step('move', async () => {
      await ui.setView('grid'); await ui.populateClipboard(source, paths, true, true, { directories: [child(source, folderName)] })
      await ui.paste(moved, child(moved, entries[0]), { menu: true })
    }, async () => { await verifyTree(fixture, source, new Map()); await verifyTree(fixture, copied, expected); await verifyTree(fixture, moved, expected) })
    await step('rename-emoji', async () => {
      await ui.navigate(moved); await ui.beginRename(child(moved, 'emoji-🧪.txt'))
      await ui.renameDraft('renamed-emoji.txt')
      expected.set('renamed-emoji.txt', expected.get('emoji-🧪.txt')); expected.delete('emoji-🧪.txt')
    }, () => verifyTree(fixture, moved, expected))
    if (target.kind === 'local') {
      for (const folder of [false, true]) await step(`create-trailing-${folder ? 'folder' : 'file'}`, async () => {
        const name = folder ? ' trailing folder ' : ' trailing file '
        await ui.emptySpace(moved); await ui.beginCreation(moved, folder); await ui.creationValue(folder, name)
        await ui.submitCreation(folder); expected.set(name, folder ? null : '')
      }, () => verifyTree(fixture, moved, expected))
      await step('rename-trailing', async () => {
        await ui.beginRename(child(moved, ' trailing file ')); await ui.renameDraft(' renamed trailing ')
        expected.delete(' trailing file '); expected.set(' renamed trailing ', '')
      }, () => verifyTree(fixture, moved, expected))
    }
    const status = await ui.handshake(plan.runId)
    assert.equal(status.cancelTasks, 0, 'No active operation remains')
  }, { id: `names-${target.kind}`, providers: [target.kind] })
}

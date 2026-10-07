import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyTree} from './editing.mjs'

export const hiddenManifest = plan => plan.targets.filter(t => t.kind !== 'cloud').map(t => ({
  id: `hidden-${t.kind}`, name: `${t.kind}: Hidden rename then delete`, providers: [t.kind],
  partIds: ['delete-without-rename', 'unhide-refresh-selection', 'delete-current-path'],
}))

export async function hidden(plan, fixture, ui, record) {
  ui.waitTimeout = 180_000
  for (const target of plan.targets.filter(t => t.kind !== 'cloud')) await record(`${target.kind}: Hidden rename then delete`, async result => {
    result.phase = 'setup'
    const base = child(target.files, 'hidden-delete'), folder = child(base, '.generated'), visible = child(base, 'generated')
    const expected = new Map([['.generated', null], ['.generated/nested.txt', 'Generated Hidden delete bytes.\n'], ['sentinel.txt', 'Keep exactly.\n']])
    await fixture.mkdir(base); await fixture.mkdir(folder)
    await fixture.write(child(folder, 'nested.txt'), expected.get('.generated/nested.txt'))
    await fixture.write(child(base, 'sentinel.txt'), expected.get('sentinel.txt'))
    const control = child(base, 'control')
    await fixture.mkdir(control); await fixture.write(child(control, 'nested.txt'), 'Generated control bytes.\n')
    expected.set('control', null); expected.set('control/nested.txt', 'Generated control bytes.\n')
    await ui.navigate(base); await ui.setView('list'); await ui.hidden(true)
    await recordPart(result, 'delete-without-rename', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'
      await ui.select(control); await ui.selection(base, [control], {directories: [control]})
      await ui.deleteSelection()
      expected.delete('control'); expected.delete('control/nested.txt')
      part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'; await verifyTree(fixture, base, expected)
    })
    await recordPart(result, 'unhide-refresh-selection', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'
      await ui.propertiesOpen(base, [folder], false, {directories: [folder]})
      const toggle = await ui.browser.$('.properties-modal input[aria-label="Hidden attribute"]')
      assert.equal(await toggle.isSelected(), true)
      await (await ui.browser.$('.properties-modal label.properties-toggle')).click()
      await ui.browser.waitUntil(async () => (await ui.propertiesRows()).Name === 'generated', {timeout: 60_000})
      await ui.closeProperties(base)
      // No F5, navigation, or reselection: the existing selection must now
      // refer to the renamed folder, and the old row must be gone.
      await ui.selection(base, [visible], {directories: [visible]})
      assert.equal(await (await ui.browser.$(`[data-path="${folder}"]`)).isExisting(), false)
      expected.delete('.generated'); expected.delete('.generated/nested.txt')
      expected.set('generated', null); expected.set('generated/nested.txt', 'Generated Hidden delete bytes.\n')
      part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'; await verifyTree(fixture, base, expected)
    })
    await recordPart(result, 'delete-current-path', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'
      await ui.deleteSelection()
      expected.delete('generated'); expected.delete('generated/nested.txt')
      part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'; await verifyTree(fixture, base, expected)
    })
  }, {id: `hidden-${target.kind}`, providers: [target.kind]})
}

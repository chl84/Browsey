import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { recordPart } from './report.mjs'

export function creationManifest(plan) {
  return plan.targets.map(target => ({ id: `creation-${target.kind}`,
    name: `${target.kind}: creation, rejection, cancellation and focus`, providers: [target.kind] }))
}

export async function creation(plan, fixture, ui, record) {
  for (const target of plan.targets) await record(`${target.kind}: owned creation edge cases`, async (result = {}) => {
    result.phase = 'setup'
    const base = child(target.files, 'creation'), existing = child(base, 'existing.txt')
    const directory = child(base, 'existing-folder'), nested = child(directory, 'preserved.txt')
    const content = 'Generated creation sentinel: preserve exactly.\n'
    await fixture.mkdir(base); await fixture.mkdir(directory)
    await fixture.write(existing, content); await fixture.write(nested, content)
    const expected = new Map([[existing, 'file'], [directory, 'dir']])
    // At most two independent read-only checks at once. Await both even if one
    // fails, so a fixture subprocess cannot outlive failure reporting/teardown.
    const pair = async (first, second) => {
      const results = await Promise.allSettled([first(), second()])
      for (const result of results) if (result.status === 'rejected') throw result.reason
      return results.map(result => result.value)
    }
    const preserve = async () => {
      result.phase = 'verification'
      const [snapshot, original] = await pair(() => fixture.snapshot(base), () => fixture.read(existing))
      assert.deepEqual(snapshot.map(entry => [entry.path, entry.kind]).sort(), [...expected].sort(), 'Creation/rejection must preserve the exact expected entries')
      assert.equal(original, content, 'Existing file must never be truncated')
      const [nestedBytes, children] = await pair(() => fixture.read(nested), () => fixture.snapshot(directory))
      assert.equal(nestedBytes, content, 'Existing nested content must survive')
      assert.deepEqual(children.map(entry => entry.path), [nested])
    }
    const step = (id, action) => recordPart(result, id, async part => {
      result.phase = 'ui'; part.ui = 'STARTED'
      await action(part)
    })
    const success = async (folder, name, button, part) => {
      await ui.beginCreation(base, folder); await ui.creationValue(folder, name)
      await ui.submitCreation(folder, { button }); part.ui = 'ACKNOWLEDGED'
      await ui.creationFocus(base)
      const path = child(base, name)
      expected.set(path, folder ? 'dir' : 'file')
      result.phase = 'verification'
      if (folder) assert.deepEqual(await fixture.snapshot(path), [], 'New folder must be empty')
      else assert.equal(await fixture.read(path), '', 'New file must contain zero bytes')
      await preserve()
    }
    result.phase = 'ui'; await ui.navigate(base); await ui.setView('list')
    await step('folder-enter-empty', part => success(true, 'created-folder', false, part))
    await step('file-button-zero-bytes', part => success(false, 'created-empty.txt', true, part))
    for (const folder of [true, false]) {
      const kind = folder ? 'folder' : 'file'
      await step(`${kind}-cancel`, async part => {
        await ui.beginCreation(base, folder); await ui.creationValue(folder, `cancel-${kind}`)
        await ui.cancelCreation(folder); part.ui = 'ACKNOWLEDGED'
        await ui.creationFocus(base); await preserve()
      })
      await step(`${kind}-escape`, async part => {
        await ui.beginCreation(base, folder); await ui.creationValue(folder, `escape-${kind}`)
        await ui.cancelCreation(folder, true); part.ui = 'ACKNOWLEDGED'
        await ui.creationFocus(base); await preserve()
      })
      for (const [label, name, error] of [['empty', '', /name cannot be empty/i], ['whitespace', '   ', /name cannot be empty/i],
        ['slash', 'invalid/name', /Invalid .* name/i], ['backslash', 'invalid\\name', /Invalid .* name/i],
        ['dot', '.', /Invalid .* name/i], ['parent-dot', '..', /Invalid .* name/i],
        ['existing-file', 'existing.txt', /already exists|destination exists/i],
        ['existing-folder', 'existing-folder', /already exists|destination exists/i]]) {
        await step(`${kind}-reject-${label}`, async part => {
          await ui.beginCreation(base, folder); await ui.creationValue(folder, name)
          await ui.submitCreation(folder, { error }); part.ui = 'ACKNOWLEDGED'
          await preserve()
          result.phase = 'ui'; await ui.cancelCreation(folder); await ui.creationFocus(base)
        })
      }
    }
    await step('grid-recover-rejected-file', async part => {
      await ui.setView('grid'); await ui.beginCreation(base, false)
      await ui.creationValue(false, 'existing.txt'); await ui.submitCreation(false, { error: /already exists|destination exists/i })
      await preserve(); result.phase = 'ui'
      await ui.creationValue(false, 'recovered.txt'); await ui.submitCreation(false)
      part.ui = 'ACKNOWLEDGED'; await ui.creationFocus(base)
      const path = child(base, 'recovered.txt'); expected.set(path, 'file')
      result.phase = 'verification'; assert.equal(await fixture.read(path), ''); await preserve()
    })
    await step('grid-folder-button-empty', part => success(true, 'grid-folder', true, part))
    result.phase = 'ui'; await ui.navigate(target.files)
  }, { id: `creation-${target.kind}`, providers: [target.kind] })
}

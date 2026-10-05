import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { payload } from './fixtures.mjs'
import { recordPart } from './report.mjs'

export function navigationManifest(plan) {
  return plan.targets.flatMap(target => ['list', 'grid'].map(view => ({
    id: `navigation-${view}-${target.kind}`, name: `${target.kind}: ${view} owned-folder navigation`,
    providers: [target.kind],
  })))
}

export async function navigation(plan, fixture, ui, record) {
  for (const target of plan.targets) for (const view of ['list', 'grid']) {
    const base = child(target.files, `navigation-${view}`)
    const branch = child(base, 'branch')
    const nested = child(branch, 'nested')
    const empty = child(base, 'empty')
    const rootFile = child(base, 'root.txt')
    const branchFile = child(branch, 'branch.txt')
    const nestedFile = child(nested, 'nested.txt')
    const refreshed = child(empty, 'after-refresh.txt')
    const id = `navigation-${view}-${target.kind}`
    await record(`${target.kind}: ${view} owned-folder navigation`, async (result = {}) => {
      result.phase = 'setup'
      for (const directory of [base, branch, nested, empty]) await fixture.mkdir(directory)
      for (const file of [rootFile, branchFile, nestedFile]) await fixture.write(file)
      const step = (name, action) => recordPart(result, name, async part => {
        result.phase = 'ui'; part.ui = 'STARTED'
        await action()
        part.ui = 'ACKNOWLEDGED'
      })
      await step('bookmark-and-view', async () => {
        await ui.navigate(base)
        await ui.setView(view)
        await ui.listing(base, view, [branch, empty, rootFile])
        await ui.bookmarks()
      })
      await step('nested-folders', async () => {
        await ui.openFolder(branch)
        await ui.listing(branch, view, [nested, branchFile])
        await ui.openFolder(nested)
        await ui.listing(nested, view, [nestedFile])
      })
      await step('back-forward', async () => {
        for (const [direction, path, contents] of [['back', branch, [nested, branchFile]],
          ['back', base, [branch, empty, rootFile]], ['forward', branch, [nested, branchFile]],
          ['forward', nested, [nestedFile]]]) {
          await ui.history(direction, path)
          await ui.listing(path, view, contents)
        }
      })
      await step('in-scope-breadcrumb', async () => {
        await ui.breadcrumb(base)
        await ui.listing(base, view, [branch, empty, rootFile])
      })
      await step('empty-folder', async () => {
        await ui.openFolder(empty)
        await ui.listing(empty, view, [])
        await ui.menuRefresh()
        await ui.listing(empty, view, [])
      })
      await step('f5-new-fixture', async () => {
        result.phase = 'setup'
        await fixture.write(refreshed)
        result.phase = 'ui'
        await ui.refresh()
        await ui.listing(empty, view, [refreshed])
      })
      await step('repeated-visits', async () => {
        for (let visit = 0; visit < 2; visit++) {
          await ui.breadcrumb(base)
          await ui.listing(base, view, [branch, empty, rootFile])
          await ui.openFolder(empty)
          await ui.listing(empty, view, [refreshed])
        }
      })
      await step('view-switch-preserves-folder', async () => {
        await ui.setView(view === 'list' ? 'grid' : 'list')
        await ui.listing(empty, view === 'list' ? 'grid' : 'list', [refreshed])
        await ui.setView(view)
        await ui.listing(empty, view, [refreshed])
      })
      result.phase = 'verification'
      for (const file of [rootFile, branchFile, nestedFile, refreshed]) assert.equal(await fixture.read(file), payload)
      for (const directory of [base, branch, nested, empty]) assert.ok(await fixture.exists(directory))
      // Leave the next case at its known owned bookmark, through a real click.
      result.phase = 'ui'
      await ui.navigate(target.files)
    }, { id, providers: [target.kind] })
  }
}

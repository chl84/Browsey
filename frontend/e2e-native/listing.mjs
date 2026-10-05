import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { recordPart } from './report.mjs'

export function listingManifest(plan) {
  return plan.targets.map(target => ({ id: `listing-${target.kind}`,
    name: `${target.kind}: sorting, filters and scoped recursive search`,
    providers: [target.kind] }))
}

// Expected order comes from independent fixture metadata, never rendered cells.
export function orderedFiles(snapshot, field, direction) {
  const key = entry => field === 'Size' ? entry.size : field === 'Modified' ? entry.modifiedMinute
    : field === 'Type' ? entry.name.split('.').at(-1).toLowerCase() : entry.name.toLowerCase()
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0
  return snapshot.filter(entry => entry.kind === 'file' && !entry.name.startsWith('.'))
    .sort((a, b) => (compare(key(a), key(b)) || compare(a.name.toLowerCase(), b.name.toLowerCase())) * (direction === 'desc' ? -1 : 1))
    .map(entry => entry.path)
}

export async function listing(plan, fixture, ui, record) {
  for (const target of plan.targets) {
    const base = child(target.files, 'listing')
    const nested = child(base, 'nested'), empty = child(base, 'empty')
    const sibling = child(target.files, 'listing-sibling')
    const files = [
      [child(base, 'Alpha.TXT'), 'a'.repeat(16)],
      [child(base, 'bravo.txt'), 'b'.repeat(16_384)],
      [child(base, 'charlie.md'), 'c'.repeat(48)],
      [child(base, 'zulu.LOG'), 'z'.repeat(96)],
      [child(base, '.hidden.txt'), 'h'.repeat(24)],
      [child(nested, 'Alpha-inner.TXT'), 'i'.repeat(32)],
      [child(nested, 'unrelated.log'), 'u'.repeat(64)],
      [child(sibling, 'Alpha-outside.txt'), 'o'.repeat(128)],
    ]
    const [alpha, bravo, charlie, zulu, hidden, inner] = files.map(([path]) => path)
    const visible = [nested, empty, alpha, bravo, charlie, zulu]
    await record(`${target.kind}: owned listing and search`, async (result = {}) => {
      result.phase = 'setup'
      for (const directory of [base, nested, empty, sibling]) await fixture.mkdir(directory)
      for (const [path, bytes] of files) await fixture.write(path, bytes)
      result.phase = 'verification'
      const snapshot = await fixture.snapshot(base)
      assert.deepEqual(snapshot.map(entry => entry.path).sort(), [...visible, hidden].sort())
      for (const [path, bytes] of files.slice(0, 5)) assert.equal(snapshot.find(entry => entry.path === path).size, Buffer.byteLength(bytes))
      const step = (name, action) => recordPart(result, name, async part => {
        result.phase = 'ui'; part.ui = 'STARTED'
        await action()
        part.ui = 'ACKNOWLEDGED'
      })
      await step('owned-baseline', async () => {
        await ui.navigate(base); await ui.setView('list'); await ui.hidden(false)
        await ui.listing(base, 'list', visible); await ui.bookmarks()
      })
      for (const field of ['Name', 'Type', 'Size', 'Modified']) await step(`sort-${field.toLowerCase()}`, async () => {
        for (const direction of ['asc', 'desc']) {
          await ui.sort(field, direction)
          await ui.listing(base, 'list', visible, { fileOrder: orderedFiles(snapshot, field, direction) })
        }
      })
      await step('name-buckets', async () => {
        await ui.columnFilter('Name', 'A–F'); await ui.columnFilter('Name', 'M–R')
        await ui.listing(base, 'list', [empty, nested, alpha, bravo, charlie])
      })
      await step('combined-type-size-modified', async () => {
        await ui.columnFilter('Type', 'txt')
        await ui.listing(base, 'list', [alpha, bravo])
        await ui.columnFilter('Size', '10–100 KB'); await ui.columnFilter('Modified', 'Today')
        await ui.listing(base, 'list', [bravo])
      })
      await step('empty-combination-and-column-resets', async () => {
        await ui.columnFilter('Name', 'A–F', false); await ui.listing(base, 'list', [])
        await ui.resetColumn('Size'); await ui.listing(base, 'list', [])
        await ui.resetColumn('Modified'); await ui.listing(base, 'list', [])
        await ui.resetColumn('Type'); await ui.listing(base, 'list', [nested])
        await ui.resetColumn('Name'); await ui.listing(base, 'list', visible)
      })
      await step('hidden-and-grid-reset', async () => {
        await ui.hidden(true); await ui.listing(base, 'list', [...visible, hidden])
        await ui.columnFilter('Name', 'Other symbols'); await ui.listing(base, 'list', [hidden])
        await ui.hidden(false); await ui.listing(base, 'list', [])
        await ui.setView('grid'); await ui.listing(base, 'grid', [])
        await ui.resetColumns(); await ui.listing(base, 'grid', visible)
        await ui.setView('list')
      })
      await step('sort-survives-view-change', async () => {
        await ui.sort('Size', 'asc')
        await ui.setView('grid')
        await ui.listing(base, 'grid', visible, { fileOrder: orderedFiles(snapshot, 'Size', 'asc') })
        await ui.setView('list')
      })
      for (const view of ['list', 'grid']) await step(`text-filter-${view}`, async () => {
        await ui.setView(view); await ui.filter(base, alpha, 'ALPHA')
        await ui.listing(base, view, [alpha])
        await ui.query('.TXT', false); await ui.listing(base, view, [alpha, bravo])
        await ui.query('no-match-generated', false); await ui.listing(base, view, [])
        await ui.exitQuery(base); await ui.listing(base, view, visible)
      })
      await step('scoped-recursive-search', async () => {
        await ui.setView('list'); await ui.search(base, 'ALPHA', true)
        await ui.listing(base, 'list', [alpha, inner])
        await ui.query('.TXT', true); await ui.listing(base, 'list', [alpha, bravo, inner])
        await ui.query('no-match-generated', true); await ui.listing(base, 'list', [])
      })
      await step('search-view-and-draft-change', async () => {
        await ui.query('alpha', true); await ui.setView('grid')
        await ui.listing(base, 'grid', [alpha, inner])
        await ui.query('unsubmitted-generated', false); await ui.listing(base, 'grid', [])
        await ui.exitQuery(base); await ui.listing(base, 'grid', visible)
      })
      await step('empty-folder-search-and-return', async () => {
        await ui.openFolder(empty); await ui.search(empty, 'ALPHA', true)
        await ui.listing(empty, 'grid', []); await ui.exitQuery(empty)
        await ui.breadcrumb(base); await ui.listing(base, 'grid', visible)
      })
      result.phase = 'verification'
      for (const [path, bytes] of files) assert.equal(await fixture.read(path), bytes, 'Generated fixture bytes must survive listing/search')
      for (const directory of [base, nested, empty, sibling]) assert.ok(await fixture.exists(directory))
      result.phase = 'ui'
      await ui.navigate(target.files)
    }, { id: `listing-${target.kind}`, providers: [target.kind] })
  }
}

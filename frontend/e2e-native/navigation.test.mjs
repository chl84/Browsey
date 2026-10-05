import assert from 'node:assert/strict'
import { test } from 'node:test'
import { validateConfig, makePlan, kinds, ownedPath } from './scope.mjs'
import { navigation, navigationManifest } from './navigation.mjs'
import { createReport, recordCase, finishReport, summarizeProviders } from './report.mjs'
import { payload } from './fixtures.mjs'

const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind =>
  [kind, kind === 'cloud' ? 'rclone://Generated/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '00000000-0000-4000-8000-000000000000')

// This independent in-memory model checks orchestration, never native acceptance.
function model({ corrupt = false, failRefresh = false } = {}) {
  const roots = plan.targets.map(target => target.files)
  const items = new Map(roots.map(root => [root, null]))
  const history = []
  const observations = []
  let index = -1, current, view = 'list'
  const visit = raw => {
    ownedPath(roots, raw)
    assert.equal(items.get(raw), null)
    history.splice(++index); history.push(raw); current = raw
  }
  const fixture = {
    mkdir: async raw => { ownedPath(roots, raw); assert.ok(!items.has(raw)); items.set(raw, null) },
    write: async raw => { ownedPath(roots, raw); assert.ok(!items.has(raw)); items.set(raw, payload) },
    read: async raw => corrupt ? 'corrupt fixture' : items.get(raw),
    exists: async raw => items.has(raw),
  }
  const ui = {
    navigate: async raw => { visit(roots.find(root => raw.startsWith(root))); if (raw !== current) visit(raw) },
    openFolder: async raw => { assert.equal(raw.slice(0, raw.lastIndexOf('/')), current); visit(raw) },
    setView: async next => { view = next },
    bookmarks: async () => {},
    breadcrumb: async raw => { assert.ok(current.startsWith(`${raw}/`)); visit(raw) },
    history: async (direction, expected) => {
      index += direction === 'back' ? -1 : 1
      current = history[index]
      assert.equal(current, expected)
      ownedPath(roots, current)
    },
    menuRefresh: async () => { if (failRefresh) throw new Error('synthetic refresh denial') },
    refresh: async () => { if (failRefresh) throw new Error('synthetic refresh denial') },
    listing: async (raw, expectedView, paths) => {
      assert.equal(current, raw); assert.equal(view, expectedView)
      const actual = [...items.keys()].filter(path => path.slice(0, path.lastIndexOf('/')) === raw).sort()
      assert.deepEqual(actual, [...paths].sort())
      observations.push({ path: current, view, empty: !actual.length })
    },
  }
  return { fixture, ui, observations }
}

test('navigation declares and independently exercises both views, empty/F5/menu Refresh/history visits on all five provider kinds', async () => {
  const manifest = navigationManifest(plan)
  const report = createReport(plan, kinds, manifest)
  const { fixture, ui, observations } = model()
  await navigation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action))
  finishReport(report); summarizeProviders(report)
  assert.equal(report.status, 'PASS')
  assert.equal(report.cases.length, 10)
  assert.ok(report.cases.every(item => item.parts.length === 8 && item.parts.every(part => part.status === 'PASS')))
  for (const provider of kinds) {
    assert.equal(report.providers[provider].status, 'PASS')
    assert.equal(report.providers[provider].counts.PASS, 2)
    for (const view of ['list', 'grid']) {
      assert.ok(report.providers[provider].capabilities[`ui-${view}-navigation`].passedBy.length)
      const root = plan.targets.find(target => target.kind === provider).files
      assert.ok(observations.some(item => item.path.startsWith(root) && item.view === view && item.empty))
    }
  }
})

test('navigation readback rejects changed source bytes even after every UI step claims success', async () => {
  const report = createReport(plan, kinds, navigationManifest(plan))
  const { fixture, ui } = model({ corrupt: true })
  await assert.rejects(navigation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action)))
  assert.equal(report.cases[0].status, 'FAIL')
  assert.equal(report.cases[0].failureKind, 'RESULT_MISMATCH')
  assert.ok(report.cases.slice(1).every(item => item.status === 'NOT_RUN'))
})

test('a navigation failure preserves prior step results, stops without retry and leaves other provider cases unrun', async () => {
  const report = createReport(plan, kinds, navigationManifest(plan))
  const { fixture, ui } = model({ failRefresh: true })
  let refreshCalls = 0
  const refresh = ui.menuRefresh
  ui.menuRefresh = async () => { refreshCalls++; await refresh() }
  await assert.rejects(navigation(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action)), /refresh denial/)
  assert.equal(refreshCalls, 1)
  assert.equal(report.cases[0].status, 'BLOCKED')
  assert.deepEqual(report.cases[0].parts.map(part => part.status), ['PASS', 'PASS', 'PASS', 'PASS', 'BLOCKED'])
  assert.ok(report.cases.slice(1).every(item => item.status === 'NOT_RUN'))
})

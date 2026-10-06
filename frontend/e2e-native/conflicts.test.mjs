import assert from 'node:assert/strict'
import { test } from 'node:test'
import { conflicts, conflictManifest } from './conflicts.mjs'
import { makePlan, validateConfig, kinds, ownedPath } from './scope.mjs'
import { createReport, recordCase, finishReport } from './report.mjs'
const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind => [kind,
  kind === 'cloud' ? 'rclone://test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
  rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '23456789-1234-4234-9234-123456789abc')

test('conflicts declare every choice, nested/same-kind and both cross-kind shapes on five providers plus the mixed cloud boundary', () => {
  const report = createReport(plan, kinds, conflictManifest(plan))
  assert.equal(report.cases.length, 56)
  assert.equal(report.cases.reduce((n, c) => n + c.parts.length, 0), 112)
  for (const kind of kinds) for (const op of ['copy', 'move']) for (const choice of ['skip', 'rename', 'overwrite', 'cancel']) {
    assert.ok(report.cases.some(c => c.id === `${op}-conflicts-${kind}-${kind}-${choice}`))
  }
  for (const op of ['copy', 'move']) for (const choice of ['skip', 'rename', 'overwrite', 'cancel']) {
    for (const route of ['local-cloud', 'cloud-local']) assert.ok(report.cases.some(c => c.id === `${op}-conflicts-${route}-${choice}`))
  }
})

test('conflict verification detects overwritten skips, cancelled mutations, damaged nested bytes and source loss without retrying', async () => {
  for (const defect of ['none', 'skip-overwrites', 'cancel-mutates', 'nested-damage', 'copy-removes-source']) {
    const items = new Map(), roots = plan.targets.map(t => t.files)
    const fixture = { mkdir: async path => { ownedPath(roots, path); items.set(path, null) },
      write: async (path, bytes) => { ownedPath(roots, path); items.set(path, bytes) }, read: async path => items.get(path),
      snapshot: async path => [...items].filter(([key]) => key.slice(0, key.lastIndexOf('/')) === path)
        .map(([path, value]) => ({ path, kind: value === null ? 'dir' : 'file' })) }
    let selected, cut, dispatches = 0
    const ui = { setView: async () => {}, populateClipboard: async (_base, paths, move) => { selected = paths; cut = move },
      paste: async (destination, _first, { conflict, expectedError }) => {
        dispatches++
        if (expectedError) return
        if (conflict === 'Cancel' && defect !== 'cancel-mutates') return
        for (const source of selected) {
          const leaf = source.slice(source.lastIndexOf('/') + 1), base = `${destination}/${leaf}`
          if (conflict === 'Skip' && items.has(base) && defect !== 'skip-overwrites') continue
          let target = base
          if (conflict === 'Auto-rename' && items.has(base)) {
            let idx = 1
            const dot = leaf.lastIndexOf('.')
            do { target = `${destination}/${dot > 0 ? `${leaf.slice(0, dot)}-${idx}${leaf.slice(dot)}` : `${leaf}-${idx}`}`; idx++ } while (items.has(target))
          }
          if (items.has(target) && (items.get(target) === null) !== (items.get(source) === null)) {
            for (const key of [...items.keys()]) if (key === target || key.startsWith(`${target}/`)) items.delete(key)
          }
          for (const [path, bytes] of [...items]) if (path === source || path.startsWith(`${source}/`)) {
            items.set(`${target}${path.slice(source.length)}`, defect === 'nested-damage' && path.endsWith('nested.txt') ? 'damaged nested bytes' : bytes)
            if (cut || defect === 'copy-removes-source') items.delete(path)
          }
        }
      } }
    const report = createReport(plan, kinds, conflictManifest(plan))
    const run = conflicts(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action))
    if (defect === 'none') { await run; finishReport(report); assert.equal(report.status, 'PASS'); assert.equal(dispatches, 112)
      assert.equal(report.cases.flatMap(c => c.parts).filter(p => p.outcome === 'EXPLICIT_REFUSAL').length, 6) }
    else { await assert.rejects(run, /Exact generated paths|Generated file bytes/); finishReport(report, Error('Verified corruption'))
      assert.equal(report.status, 'FAIL'); assert.ok(dispatches < 112)
      assert.ok(report.cases.some(c => c.status === 'NOT_RUN')) }
  }
})

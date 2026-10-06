import assert from 'node:assert/strict'
import { test } from 'node:test'
import { transfers, transferManifest, transferPartIds, transferRoutes } from './transfers.mjs'
import { makePlan, validateConfig, kinds, ownedPath } from './scope.mjs'
import { createReport, recordCase, finishReport } from './report.mjs'

const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind => [kind,
  kind === 'cloud' ? 'rclone://test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
  rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '23456789-1234-4234-9234-123456789abc')

test('within-provider transfer manifest declares both operations and every verification part on all five providers', () => {
  const report = createReport(plan, kinds, transferManifest(plan))
  assert.equal(report.cases.length, 10)
  for (const kind of kinds) for (const op of ['copy', 'move']) {
    const item = report.cases.find(c => c.id === `${op}-rich-${kind}-${kind}`)
    assert.deepEqual(item.providers, [kind]); assert.deepEqual(item.parts.map(p => p.id), transferPartIds)
    assert.ok(item.parts.every(p => p.status === 'NOT_RUN'))
    assert.ok(item.requirements[kind].includes('independent-source-destination-trees'))
  }
  assert.throws(() => createReport(plan, kinds, [{ ...transferManifest(plan)[0], partIds: ['file', 'file'] }]), /Unique bounded/)
})

test('transfer acceptance checks both trees after each UI acknowledgement and stops on missing output, damage or early removal', async () => {
  for (const defect of ['none', 'missing-output', 'damaged-nested', 'source-removed']) {
    const roots = plan.targets.map(t => t.files), items = new Map()
    const fixture = {
      mkdir: async path => { ownedPath(roots, path); items.set(path, null) },
      write: async (path, bytes) => { ownedPath(roots, path); items.set(path, bytes) },
      read: async path => items.get(path),
      snapshot: async path => [...items].filter(([key]) => key.slice(0, key.lastIndexOf('/')) === path)
        .map(([path, value]) => ({ path, kind: value === null ? 'dir' : 'file' })),
    }
    let selected, move, dispatches = 0
    const ui = { setView: async () => {}, populateClipboard: async (_base, paths, cut) => { selected = paths; move = cut },
      paste: async destination => {
        dispatches++
        if (defect === 'missing-output') return
        for (const [path, bytes] of [...items]) {
          const source = selected.find(src => src === path || path.startsWith(`${src}/`))
          if (!source) continue
          const target = `${destination}/${source.slice(source.lastIndexOf('/') + 1)}${path.slice(source.length)}`
          items.set(target, defect === 'damaged-nested' && path.endsWith('nested.txt') ? 'damaged' : bytes)
          if (move || defect === 'source-removed') items.delete(path)
        }
      },
    }
    const report = createReport(plan, kinds, transferManifest(plan))
    const run = transfers(plan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action))
    if (defect === 'none') {
      await run; finishReport(report)
      assert.equal(report.status, 'PASS'); assert.equal(dispatches, 40)
    } else {
      await assert.rejects(run, /Exact generated paths/); finishReport(report, Error('Observed mismatch'))
      assert.equal(report.status, 'FAIL'); assert.ok(report.cases.slice(1).every(c => c.status === 'NOT_RUN'))
      assert.equal(dispatches, defect === 'damaged-nested' ? 3 : 1)
    }
  }
})

test('local-hub declares both directions and operations for every external provider with mixed-tree verification', () => {
  const routes = transferRoutes(plan, 'hub'), report = createReport(plan, kinds, transferManifest(plan, 'hub'))
  assert.equal(routes.length, 8); assert.equal(report.cases.length, 16)
  for (const kind of kinds.filter(kind => kind !== 'local')) for (const [from, to] of [['local', kind], [kind, 'local']]) {
    assert.ok(routes.some(route => route.from === from && route.to === to))
    for (const op of ['copy', 'move']) {
      const item = report.cases.find(c => c.id === `${op}-rich-${from}-${to}`)
      assert.deepEqual(item.providers, [from, to]); assert.deepEqual(item.parts.map(p => p.id), ['mixed-batch'])
      assert.ok(item.requirements[from].includes('directory-read'))
      assert.ok(item.requirements[to].includes('directory-create'))
      assert.equal(item.requirements[from].includes('source-removal'), op === 'move')
    }
  }
})

test('hub rejects stale contents from a previous route and moves which leave empty source directories', async () => {
  const selectedPlan = { ...plan, targets: plan.targets.filter(t => ['local', 'usb'].includes(t.kind)) }
  for (const defect of ['none', 'stale-copy', 'move-files-only']) {
    const roots = selectedPlan.targets.map(t => t.files), items = new Map()
    const fixture = { mkdir: async path => { ownedPath(roots, path); items.set(path, null) },
      write: async (path, bytes) => { ownedPath(roots, path); items.set(path, bytes) },
      read: async path => items.get(path), snapshot: async path => [...items].filter(([key]) => key.slice(0, key.lastIndexOf('/')) === path)
        .map(([path, value]) => ({ path, kind: value === null ? 'dir' : 'file' })) }
    let paths, cut, previousCopyBytes, dispatches = 0
    const ui = { setView: async () => {}, populateClipboard: async (_base, selected, move) => { paths = selected; cut = move },
      paste: async target => {
        dispatches++
        for (const [path, bytes] of [...items]) {
          const source = paths.find(src => src === path || path.startsWith(`${src}/`))
          if (!source) continue
          const relative = `${source.slice(source.lastIndexOf('/') + 1)}${path.slice(source.length)}`
          const stale = defect === 'stale-copy' && !cut && previousCopyBytes && relative === 'mixed.txt'
          items.set(`${target}/${relative}`, stale ? previousCopyBytes : bytes)
          if (!cut && relative === 'mixed.txt' && !previousCopyBytes) previousCopyBytes = bytes
          if (cut && !(defect === 'move-files-only' && bytes === null)) items.delete(path)
        }
      } }
    const report = createReport(selectedPlan, ['local', 'usb'], transferManifest(selectedPlan, 'hub'))
    const run = transfers(selectedPlan, fixture, ui, (_name, action, metadata) => recordCase(report, metadata, action), 'hub')
    if (defect === 'none') { await run; finishReport(report); assert.equal(report.status, 'PASS'); assert.equal(dispatches, 4) }
    else {
      await assert.rejects(run, /Exact generated paths/); finishReport(report, Error('Observed mismatch'))
      assert.equal(report.status, 'FAIL'); assert.equal(dispatches, defect === 'stale-copy' ? 3 : 2)
      assert.ok(report.cases.slice(dispatches).every(c => c.status === 'NOT_RUN'))
    }
  }
})

import assert from 'node:assert/strict'
import {test} from 'node:test'
import {suites} from './catalog.mjs'
import {suiteTier} from './tiers.mjs'
import {acceptanceRows} from './acceptance-map.mjs'
import {makePlan, validateConfig} from './scope.mjs'
import {createReport} from './report.mjs'
const roots = {local: '/test/ai_agent_testfolder', usb: '/usb/ai_agent_testfolder', network: '/network/ai_agent_testfolder', cloud: 'rclone://Test/ai_agent_testfolder', mobile: '/mobile/ai_agent_testfolder'}
const specific = {usb: ['local', 'usb'], 'usb-access': ['local', 'usb'], network: ['local', 'network'], mobile: ['local', 'mobile'], 'cloud-provider': ['local', 'cloud'], 'cloud-working': ['local', 'cloud'], 'storage-performance': ['local', 'usb', 'mobile', 'cloud'], 'cloud-export': ['local', 'cloud'], 'cloud-scale': ['local', 'cloud'], 'cloud-race': ['local', 'cloud'], 'cloud-trash': ['local', 'cloud'], 'cloud-drag':['local','cloud']}
const local = ['smoke', 'repeatability', 'measurements', 'interruption', 'links', 'drag', 'drag-feedback', 'desktop-services', 'keyboard', 'appearance', 'watchers', 'archives', 'open-with']
test('every executable suite has a tier and every declared case maps to valid acceptance rows', () => {
  const ids = new Set()
  for (const [suite, entry] of Object.entries(suites)) {
    const kinds = specific[suite] ?? (local.includes(suite) ? ['local'] : Object.keys(roots))
    const config = validateConfig({schema: 1, targets: Object.fromEntries(kinds.map(kind => [kind, roots[kind]])), rcloneConfig: kinds.includes('cloud') ? '/test/ai_agent_testfolder/rclone.conf' : null, matrix: 'all-pairs'})
    if(suite==='cloud-drag') {config.targets.find(t=>t.kind==='cloud').path='rclone://Onedrive/ai_agent_testfolder';config.targets.push({kind:'cloud',path:'rclone://Google Disk/ai_agent_testfolder'})}
    const plan = makePlan(config, '00000000-0000-4000-8000-000000000000')
    const manifest = entry.manifest(plan)
    assert.ok(suiteTier(suite))
    createReport(plan, kinds, manifest) // Validate declarations/parts; not native execution evidence.
    for (const item of manifest) {
      ids.add(item.id)
      assert.ok(acceptanceRows(item.sourceCaseId ?? item.id).every(row => /^A[0-4]-[1-8]$/.test(row)))
    }
  }
  assert.ok(ids.size > 300, 'All configured provider/direction families must be included')
  assert.deepEqual(acceptanceRows('lifecycle-owned-window'), ['A0-4', 'A2-6'])
  assert.throws(() => acceptanceRows('unknown-new-case'))
})
test('mappings add supporting evidence without marking any parent acceptance row passed', () => {
  const rows = acceptanceRows('desktop-appearance')
  assert.deepEqual(rows, ['A2-3', 'A2-4', 'A3-2'])
  rows.push('invented')
  assert.deepEqual(acceptanceRows('desktop-appearance'), ['A2-3', 'A2-4', 'A3-2'])
})

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { parseRouting } from './routing.mjs'
import { makePlan, validateConfig, kinds } from './scope.mjs'
import { transferRoutes, transferManifest } from './transfers.mjs'

test('all-pairs explicitly reruns every ordered route, including the eight hub directions', () => {
  const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind => [kind,
    kind === 'cloud' ? 'rclone://test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
  rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '23456789-1234-4234-9234-123456789abc')
  const hub = transferRoutes(plan, 'hub'), pairs = transferRoutes(plan, 'pairs')
  assert.equal(hub.length, 8); assert.equal(pairs.length, 20); assert.equal(transferManifest(plan, 'pairs').length, 40)
  assert.ok(pairs.every(p => p.from !== p.to))
  assert.equal(new Set([...hub, ...pairs].map(p => `${p.from}-${p.to}`)).size, 20)
  for (const from of kinds) for (const to of kinds.filter(k => k !== from)) {
    assert.ok([...hub, ...pairs].some(p => p.from === from && p.to === to))
  }
})

test('routing evidence rejects missing, unknown and excessive receipts and returns only declared fields', () => {
  const line = (kind, backend = 'rclone-cli') => `INFO transfer dispatch op="copy" backend="${backend}" kind="${kind}" staging="direct" private-path-secret`
  const valid = [line('file', 'rclone-provider-upload'), line('directory'), line('directory')].join('\n')
  assert.equal(parseRouting(valid, 'copy').events.length, 3)
  assert.ok(!JSON.stringify(parseRouting(valid, 'copy')).includes('secret'))
  assert.throws(() => parseRouting(valid, 'move'), /Missing/)
  assert.throws(() => parseRouting(line('file').repeat(3), 'copy'), /Missing/)
  assert.throws(() => parseRouting(valid.replace('rclone-cli', 'unknown'), 'copy'), /Unknown/)
  assert.throws(() => parseRouting(Array(65).fill(line('file')).join('\n'), 'copy'), /excessive/)
})

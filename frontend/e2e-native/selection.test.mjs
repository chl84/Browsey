import assert from 'node:assert/strict'
import { test } from 'node:test'
import { selectionManifest, verifySelectedCopy } from './selection.mjs'
import { createReport, recordCase, recordPart } from './report.mjs'
import { makePlan, validateConfig, kinds } from './scope.mjs'

test('selection scope declares every configured provider and local virtualization separately', () => {
  const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind =>
    [kind, kind === 'cloud' ? 'rclone://Generated/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
  rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '00000000-0000-4000-8000-000000000000')
  const report = createReport(plan, kinds, selectionManifest(plan))
  assert.equal(report.cases.length, 6)
  for (const kind of kinds) assert.ok(report.cases.some(item => item.id === `selection-${kind}`))
  assert.deepEqual(report.cases.filter(item => Object.values(item.requirements).some(caps => caps.includes('virtualized-selection')))
    .map(item => item.id), ['selection-virtual-local'])
  assert.ok(!report.cases.at(-1).requirements.local.includes('ui-empty-space'))
  assert.ok(!report.cases.at(-1).requirements.local.includes('selection-navigation'))
})

test('independent selected-copy verification rejects missing, extra, duplicated and corrupted outputs', async () => {
  const paths = ['/owned/source/a.txt', '/owned/source/c.txt']
  const bytes = new Map([[paths[0], 'generated a'], [paths[1], 'generated c']])
  const expected = ['/owned/dest/a.txt', '/owned/dest/c.txt']
  for (const output of [expected.slice(1), [...expected, '/owned/dest/b.txt'], [...expected, expected[0]]]) {
    await assert.rejects(verifySelectedCopy({ snapshot: async () => output.map(path => ({ path })) }, '/owned/dest', paths, bytes), /exactly/)
  }
  const fixture = { snapshot: async () => expected.map(path => ({ path })), read: async () => 'corrupted' }
  const report = { cases: [{ id: 'selection-local', providers: ['local'], parts: [], status: 'NOT_RUN' }] }
  await assert.rejects(recordCase(report, { id: 'selection-local', providers: ['local'] }, async result => {
    await recordPart(result, 'copy', async part => {
      part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'
      await verifySelectedCopy(fixture, '/owned/dest', paths, bytes)
    })
  }), /bytes must match/)
  assert.equal(report.cases[0].status, 'FAIL')
  assert.equal(report.cases[0].failureKind, 'RESULT_MISMATCH')
  assert.equal(report.cases[0].parts[0].ui, 'ACKNOWLEDGED')
})

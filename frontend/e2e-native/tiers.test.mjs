import assert from 'node:assert/strict'
import {test} from 'node:test'
import {selectTier, smoke, smokeManifest} from './tiers.mjs'
import {selectedFoundationManifest} from './cases.mjs'
const plan = {targets: [{kind: 'local', files: '/owned/files'}], routes: []}
test('tiers require explicit stress/edge/lifecycle members and reject cross-tier execution', () => {
  assert.equal(selectTier('smoke', null), 'smoke')
  assert.equal(selectTier('provider', null), 'foundation')
  for (const tier of ['edge', 'stress', 'lifecycle']) assert.throws(() => selectTier(tier, null))
  assert.throws(() => selectTier('smoke', 'cloud-provider'))
  assert.throws(() => selectTier('provider', 'interruption'))
  assert.throws(() => selectTier('edge', 'foundation', 'driver-exit'))
  assert.equal(selectTier('lifecycle', 'foundation', 'driver-exit'), 'foundation')
})
test('smoke refuses providers and foundation selections enforce dependencies', () => {
  assert.throws(() => smokeManifest({...plan, targets: [...plan.targets, {kind: 'cloud'}]}))
  assert.throws(() => selectedFoundationManifest(plan, ['rename-local']))
  assert.throws(() => selectedFoundationManifest(plan, ['create-local', 'delete-local']))
  assert.throws(() => selectedFoundationManifest(plan, ['unknown']))
})
test('smoke executes exactly its declared existing bodies; unused fixtures are not created', async () => {
  const actual = []
  // Policy/mock evidence only: actions are deliberately not evaluated here.
  await smoke(plan, {}, {}, async (name, action, metadata) => actual.push(metadata.id))
  assert.deepEqual(actual, smokeManifest(plan).map(item => item.id))
})

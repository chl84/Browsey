import assert from 'node:assert/strict'
import {foundation, selectedFoundationManifest} from './cases.mjs'

export const smokeIds = Object.freeze(['input-local', 'copy-within-local', 'undo-copy-local'])
export function smokeManifest(plan) {
  assert.deepEqual(plan.targets.map(target => target.kind), ['local'], 'Smoke requires explicit local scope')
  return selectedFoundationManifest(plan, smokeIds)
}
export async function smoke(plan, fixture, ui, record) {
  smokeManifest(plan)
  await foundation(plan, fixture, ui, record, smokeIds)
}

// Plans select existing bodies. No tier implicitly runs providers or stress.
export const tiers = Object.freeze({
  smoke: ['smoke', 'repeatability'],
  provider: ['foundation', 'transfers-within', 'transfers-hub', 'transfers-pairs', 'usb', 'usb-access', 'network', 'mobile', 'cloud-provider', 'cloud-working'],
  edge: ['navigation', 'listing', 'selection', 'creation', 'editing', 'fileops', 'rename', 'properties', 'history', 'guards', 'guards-aliases-mobile', 'batches', 'conflicts', 'progress', 'cancellation', 'overwrite', 'moves', 'access', 'iofaults', 'races', 'names', 'links', 'drag', 'drag-feedback', 'keyboard', 'appearance', 'watchers', 'archives', 'open-with', 'desktop-services', 'cloud-export', 'cloud-race', 'cloud-trash'],
  stress: ['measurements', 'storage-performance', 'cloud-scale', 'contents', 'trees', 'limits'],
  lifecycle: ['interruption'],
})
export function selectTier(tier, suite, fault = null) {
  assert.ok(Object.hasOwn(tiers, tier), 'Unknown native tier')
  if (fault) { assert.equal(tier, 'lifecycle'); assert.equal(suite, 'foundation'); return suite }
  const selected = suite ?? ({smoke: 'smoke', provider: 'foundation'}[tier])
  assert.ok(tiers[tier].includes(selected), 'Tier requires an explicit member suite')
  return selected
}
export function suiteTier(suite, fault = null) {
  if (fault) return 'lifecycle'
  const tier = Object.keys(tiers).find(tier => tiers[tier].includes(suite))
  assert.ok(tier, 'Every native suite must have an evidence tier')
  return tier
}

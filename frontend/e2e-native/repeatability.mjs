import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {smoke, smokeManifest} from './tiers.mjs'
import {releasedResources} from './resources.mjs'
import {verifyTree} from './editing.mjs'
import {payload} from './fixtures.mjs'
import {settingsChoice} from './appearance.mjs'
const phases = ['fresh', 'reused']
export function repeatabilityManifest(plan) {
  return [...phases.flatMap(phase => smokeManifest(plan).map(item => ({...item,
    sourceCaseId: item.id, id: `${item.id}-${phase}`, name: `${phase}: ${item.name}`}))),
  {id: 'lifecycle-profile-reuse', name: 'One owned restart with persisted density', providers: ['local']}]
}
export async function repeatability(plan, fixture, ui, record) {
  repeatabilityManifest(plan)
  for (const phase of phases) {
    const files = child(plan.targets[0].files, phase)
    await fixture.mkdir(files)
    const phasePlan = {...plan, targets: [{...plan.targets[0], files}]}
    await smoke(phasePlan, fixture, ui, (name, action, metadata) => record(`${phase}: ${name}`, action,
      {...metadata, sourceCaseId: metadata.id, id: `${metadata.id}-${phase}`}))
    await releasedResources(ui, plan.runId)
    if (phase === 'fresh') await record('One owned restart with persisted density', async result => {
      result.phase = 'ui'
      await settingsChoice(ui, 'Density', 'Compact')
      await ui.restart()
      await ui.idle()
      assert.ok((await ui.browser.execute(() => document.body.className)).includes('density-compact'))
      result.phase = 'verification'
      await verifyTree(fixture, child(files, 'copy-within-local-source'),
        new Map([['sample.txt', payload], ['tree', null], ['tree/nested.txt', payload]]))
      result.profile = 'Same exclusive run/profile; one restart after confirmed teardown'
      result.persistedDensity = 'compact'
      result.resources = await releasedResources(ui, plan.runId)
    }, {id: 'lifecycle-profile-reuse', providers: ['local']})
    await verifyTree(fixture, child(files, 'undo-target'), new Map([['undo.txt', payload]]))
  }
}
/* global document */

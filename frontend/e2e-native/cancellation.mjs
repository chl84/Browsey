/* global window */
import assert from 'node:assert/strict'
import {performance} from 'node:perf_hooks'
import {child} from './scope.mjs'
import {verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {progressRoutes,beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {measuredSamples,cancelFeedbackTimings} from './metrics.mjs'

export const cancellationCases=plan=>[
  ...progressRoutes(plan).map(c=>({...c,point:'before',phase:[c.from,c.to].includes('cloud')?'validation':'start'})),
  {from:'local',to:'local',point:'middle',phase:'written'},
  ...progressRoutes(plan).filter(c=>c.from===c.to||[c.from,c.to].includes('cloud'))
    .map(c=>({...c,point:'between',phase:[c.from,c.to].includes('cloud')?'validation':'start'})),
]
const idFor=c=>`cancel-${c.from}-${c.to}-${c.point}`
export const cancellationManifest=plan=>cancellationCases(plan).map(c=>({id:idFor(c),
  name:`Copy cancellation: ${c.from} to ${c.to}, ${c.point}`,providers:[...new Set([c.from,c.to])],partIds:['outcome']}))
export function cancellationLocations(plan,c) {
  const id=idFor(c),from=child(plan.targets.find(t=>t.kind===c.from).files,`${id}-source`),
    to=child(plan.targets.find(t=>t.kind===c.to).files,`${id}-target`),name=c.point==='between'?'b-stop.txt':'a-stop.txt'
  return {id,from,to,name,source:child(from,name),target:child(to,name)}
}
export const cancellationProbes=plan=>cancellationCases(plan).map(c=>{
  const p=cancellationLocations(plan,c)
  return {id:p.id,source:p.source,target:p.target,holdPhase:c.phase,holdBytes:c.point==='middle'?16384:0,holdMs:5000,slowMs:0}
})

// Read real callback registries, without dispatching an event or replacing APIs.
export async function transferListeners(ui) {
  return ui.browser.execute(()=>{
    const listeners=window.__internal_unstable_listeners_object_id__,callbacks=window.__TAURI_INTERNALS__?.callbacks
    if(!listeners||typeof callbacks?.has!=='function') throw Error('Native event registries are unavailable')
    return Object.getOwnPropertyNames(listeners).filter(name=>/^(copy-progress-|cut-progress-|mixed-(copy|cut)-|cloud-(copy|cut)-|compress-progress-|extract-progress-)/.test(name))
      .flatMap(name=>Object.getOwnPropertyNames(listeners[name]).map(id=>listeners[name][id].handlerId)
        .filter(id=>callbacks.has(id)).map(id=>({event:name,handler:id})))
  })
}
export async function heldProbe(ui,runId,id) {
  let status,probe
  await ui.browser.waitUntil(async()=>{
    status=await ui.handshake(runId);probe=status.probes.find(p=>p.id===id)
    return probe?.consumed&&probe.observation.held
  },{timeout:180_000,interval:50,timeoutMsg:'Real transfer did not reach its owned checkpoint'})
  assert.equal(status.cancelTasks,1,'The visible cancellable task must already be registered')
  assert.equal((await transferListeners(ui)).length,1,'The active transfer owns one progress callback')
  return probe
}
export function assertCancelledOutput(content,actual) {
  assert.ok(actual.length>0&&actual.length<content.length,'Cancelled mid-file output cannot be complete')
  assert.equal(actual,content.slice(0,actual.length),'Retained partial output must contain only its source prefix')
}
export async function cancelHeld(ui,plan,id) {
  const checkpoint=await heldProbe(ui,plan.runId,id)
  const started=performance.now()
  await (await ui.browser.$('[aria-label="Cancel task"]')).click()
  const feedback=await ui.expectedToast(/Paste failed:.*cancel/i,600_000)
  await waitActivityGone(ui)
  const observations=await endTransferObservation(ui)
  assert.ok(observations.some(s=>s.label==='Cancelling…'),'The real Cancel button must enter cancelling state')
  assert.equal((await ui.handshake(plan.runId)).cancelTasks,0,'Finished cancel tasks must be released')
  assert.deepEqual(await transferListeners(ui),[],'Finished transfer callbacks must be unregistered')
  return {checkpoint,feedback,observations,latency:cancelFeedbackTimings(observations),verificationRoundTrip:measuredSamples([performance.now()-started],
    'Node monotonic clock: full verification including WebDriver, readiness polling and waiting for the terminal toast to expire; not cancellation latency')}
}
export async function cancellations(plan,fixture,ui,record) {
  for(const c of cancellationCases(plan)) await record(`Copy cancellation: ${c.from} to ${c.to}, ${c.point}`,async result=>{
    await ui.setView('list')
    await recordPart(result,'outcome',async part=>{
      result.phase='setup'
      const p=cancellationLocations(plan,c),source=new Map(),target=new Map(),names=c.point==='between'
        ?['a-good.txt','b-stop.txt','c-after.txt']:['a-stop.txt']
      await fixture.mkdir(p.from);await fixture.mkdir(p.to)
      for(const name of [...names,'sentinel.txt']) {
        const content=name==='sentinel.txt'?'unrelated source\n':`${p.id}/${name}\n`.padEnd(65536,'x')
        await fixture.write(child(p.from,name),content);source.set(name,content)
      }
      await fixture.write(child(p.to,'sentinel.txt'),'unrelated target\n');target.set('sentinel.txt','unrelated target\n')
      await ui.populateClipboard(p.from,names.map(n=>child(p.from,n)),false,false)
      result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to)
      Object.assign(part,await cancelHeld(ui,plan,p.id));part.ui='ACKNOWLEDGED'
      result.phase='verification'
      if(c.point==='between'&&[c.from,c.to].includes('cloud')) {
        target.set('a-good.txt',source.get('a-good.txt'));assert.match(part.feedback,/1 completed, 0 skipped, 1 failed, 1 not attempted/)
      } else if(c.point==='between') assert.match(part.feedback,/0 completed, 0 skipped, 1 failed, 1 not attempted, 1 rolled back/)
      else assert.match(part.feedback,/0 completed, 0 skipped, 1 failed, 0 not attempted/)
      if(c.point==='middle'&&await fixture.exists(p.target)) {
        const content=await fixture.read(p.target);assertCancelledOutput(source.get(p.name),content);target.set(p.name,content)
        assert.match(part.feedback,/retained|incomplete/i);part.destination='RETAINED_INCOMPLETE_PREFIX'
      } else part.destination='NO_CANCELLED_ENTRY'
      await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      // Read independently again after quiescence to reject continued writes.
      await new Promise(resolve=>setTimeout(resolve,500))
      await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      assert.equal((await ui.handshake(plan.runId)).cancelTasks,0)
      assert.deepEqual(await transferListeners(ui),[])
      part.verification='INDEPENDENT_MATCH';part.writesStopped=true
    })
  },{id:idFor(c),providers:[...new Set([c.from,c.to])]})
}

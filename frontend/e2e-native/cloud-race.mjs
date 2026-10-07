import assert from 'node:assert/strict'
import {execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {child,ownedPath} from './scope.mjs'
import {rclonePath} from './fixtures.mjs'
import {recordPart} from './report.mjs'
import {verifyTree} from './editing.mjs'
import {releasedResources} from './resources.mjs'
import {beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {heldProbe} from './cancellation.mjs'
const exec=promisify(execFile)
export const cloudRaceManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local','cloud']);return [{id:'races-cloud-destination-finalize',name:'OneDrive destination writer before client completion',providers:['local','cloud'],partIds:['outcome']}]}
export function cloudRaceLocations(plan){const from=child(plan.targets[0].files,'cloud-race-source'),to=child(plan.targets[1].files,'cloud-race-target'),actor=child(plan.targets[0].files,'cloud-race-actor');return {from,to,actor,source:child(from,'body.txt'),target:child(to,'body.txt'),competitor:child(actor,'body.txt')}}
export const cloudRaceProbes=plan=>{const p=cloudRaceLocations(plan);return [{id:'cloud-destination-finalize',source:p.source,target:p.target,holdPhase:'finalize',holdBytes:0,holdMs:5000,slowMs:0}]}
export async function cloudRace(plan,fixture,ui,record){
  cloudRaceManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  await record('OneDrive destination writer before client completion',async result=>{
    await recordPart(result,'outcome',async part=>{
      result.phase='setup';const p=cloudRaceLocations(plan),original='generated original upload\n',foreign='generated competing target\n'
      for(const dir of [p.from,p.to,p.actor])await fixture.mkdir(dir)
      await fixture.write(p.source,original);await fixture.write(p.competitor,foreign);await fixture.write(child(p.to,'sentinel.txt'),'unrelated owned sentinel\n')
      await ui.populateClipboard(p.from,[p.source],false,false);result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to)
      part.before=await heldProbe(ui,plan.runId,'cloud-destination-finalize')
      ownedPath([plan.targets[0].files],p.competitor);ownedPath([plan.targets[1].files],p.target)
      try {await exec('/usr/bin/rclone',['--config',fixture.env.RCLONE_CONFIG,'--retries','1','--low-level-retries','1','--timeout','60s','--contimeout','15s','copyto',p.competitor,rclonePath(p.target),'--ignore-times'],{env:fixture.env,cwd:plan.targets[0].files,timeout:120_000,maxBuffer:65536})}
      catch{throw Object.assign(new Error('Exact competing cloud writer failed; no replay'),{failureKind:'FIXTURE_IO'})}
      part.after=(await ui.handshake(plan.runId)).probes.find(p=>p.id==='cloud-destination-finalize')
      assert.ok(part.after?.observation.held,'Competing cloud write must finish during the actual held completion window')
      await ui.idle({resultPath:p.target},600_000);await waitActivityGone(ui);part.progress=await endTransferObservation(ui)
      result.phase='verification';await verifyTree(fixture,p.from,new Map([['body.txt',original]]));await verifyTree(fixture,p.actor,new Map([['body.txt',foreign]]));await verifyTree(fixture,p.to,new Map([['body.txt',foreign],['sentinel.txt','unrelated owned sentinel\n']]))
      await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_CONCURRENT_TARGET_AND_SOURCE_BYTES';part.boundary='Copy acknowledged after successful upload even when another server-side writer replaces the target before client completion; requires stable destination, no provider CAS or transaction claim';part.cut=false
    })
  },{id:'races-cloud-destination-finalize',providers:['local','cloud']})
}

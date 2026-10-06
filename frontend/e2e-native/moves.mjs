import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {denyOwnedRead} from './batch.mjs'
import {progressRoutes,beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {cancelHeld,assertCancelledOutput,transferListeners} from './cancellation.mjs'

export const moveCases=plan=>[
  ...[...progressRoutes(plan),...plan.targets.filter(t=>t.kind!=='local').map(t=>({from:t.kind,to:t.kind}))]
    .map(c=>({...c,point:'before',phase:c.from!==c.to&&[c.from,c.to].includes('cloud')?'validation':'start'})),
  {from:'local',to:'usb',point:'middle',phase:'written'},
  ...[{from:'local',to:'usb'},{from:'local',to:'cloud'},{from:'cloud',to:'local'}].map(c=>({...c,point:'late',phase:'finalize'})),
  ...[{from:'local',to:'local'},{from:'local',to:'cloud'},{from:'cloud',to:'local'}]
    .map(c=>({...c,point:'between',phase:[c.from,c.to].includes('cloud')?'validation':'start'})),
  {from:'local',to:'usb',point:'failed',denied:true},
  {from:'cloud',to:'local',point:'failed',sourceFault:true},
].filter(c=>[c.from,c.to].every(k=>plan.targets.some(t=>t.kind===k)))
const idFor=c=>`moves-${c.from}-${c.to}-${c.point}`
export const moveManifest=plan=>moveCases(plan).map(c=>({id:idFor(c),name:`Move safety: ${idFor(c)}`,
  providers:[...new Set([c.from,c.to])],partIds:['outcome']}))
export function moveLocations(plan,c) {
  const id=idFor(c),from=child(plan.targets.find(t=>t.kind===c.from).files,`${id}-source`),
    to=child(plan.targets.find(t=>t.kind===c.to).files,`${id}-target`),name=['between','failed'].includes(c.point)?'b-stop.txt':'a-stop.txt'
  return {id,from,to,name,source:child(from,name),target:child(to,name)}
}
export const moveProbes=plan=>moveCases(plan).filter(c=>c.point!=='failed').map(c=>{
  const p=moveLocations(plan,c)
  return {id:p.id,source:p.source,target:p.target,holdPhase:c.phase,holdBytes:c.point==='middle'?16384:0,holdMs:5000,slowMs:0}
})
export const moveFaults=plan=>moveCases(plan).filter(c=>c.sourceFault).map(c=>({id:idFor(c),operation:'move',source:moveLocations(plan,c).source}))
export async function moves(plan,fixture,ui,record) {
  for(const c of moveCases(plan)) await record(`Move safety: ${idFor(c)}`,async result=>{
    await ui.setView('list')
    await recordPart(result,'outcome',async part=>{
      result.phase='setup'
      const p=moveLocations(plan,c),source=new Map(),target=new Map(),names=['between','failed'].includes(c.point)
        ?['a-good.txt','b-stop.txt','c-after.txt']:['a-stop.txt']
      await fixture.mkdir(p.from);await fixture.mkdir(p.to)
      for(const name of [...names,'sentinel.txt']) {
        const bytes=name==='sentinel.txt'?'unrelated source\n':`${p.id}/${name}\n`.padEnd(c.denied&&name==='b-stop.txt'?4096:65536,'x')
        await fixture.write(child(p.from,name),bytes);source.set(name,bytes)
      }
      await fixture.write(child(p.to,'sentinel.txt'),'unrelated target\n');target.set('sentinel.txt','unrelated target\n')
      await ui.populateClipboard(p.from,names.map(n=>child(p.from,n)),true,false)
      const restore=c.denied?await denyOwnedRead(fixture.roots,p.source):null
      try {
        result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to)
        if(c.point==='failed') {
          part.feedback=await ui.expectedToast(c.sourceFault?/Paste failed:.*Injected owned source failure/:/Paste failed:.*(?:denied|permission)/i,600_000)
          await waitActivityGone(ui);part.observations=await endTransferObservation(ui)
        } else Object.assign(part,await cancelHeld(ui,plan,p.id))
        part.ui='ACKNOWLEDGED'
      } finally {if(restore) await restore()}
      result.phase='verification'
      if(['between','failed'].includes(c.point)&&[c.from,c.to].includes('cloud')) {
        target.set('a-good.txt',source.get('a-good.txt'));source.delete('a-good.txt')
        assert.match(part.feedback,/1 completed, 0 skipped, 1 failed, 1 not attempted/)
      } else if(['between','failed'].includes(c.point)) assert.match(part.feedback,/0 completed, 0 skipped, 1 failed, 1 not attempted, 1 rolled back/)
      else assert.match(part.feedback,/0 completed, 0 skipped, 1 failed, 0 not attempted/)
      if(c.point==='late') {
        target.set(p.name,source.get(p.name));assert.match(part.feedback,/retained.*source.*not removed/i)
        part.destination='COMPLETE_COPY_WITH_SOURCE_RETAINED'
      } else if(c.point==='middle') {
        const actual=await fixture.read(p.target);assertCancelledOutput(source.get(p.name),actual);target.set(p.name,actual)
        assert.match(part.feedback,/retained|incomplete/i);part.destination='INCOMPLETE_COPY_WITH_SOURCE_RETAINED'
      } else part.destination='NO_FAILED_ENTRY'
      await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      await new Promise(resolve=>setTimeout(resolve,500))
      await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      assert.equal((await ui.handshake(plan.runId)).cancelTasks,0);assert.deepEqual(await transferListeners(ui),[])
      await ui.cutClipboard(p.from,names.filter(n=>source.has(n)).map(n=>child(p.from,n)))
      if(c.sourceFault) {
        const fault=(await ui.handshake(plan.runId)).faults.find(f=>f.id===p.id)
        assert.equal(fault?.uses,1);part.fault={uses:fault.uses}
      }
      part.verification='INDEPENDENT_MATCH';part.writesStopped=true
    })
  },{id:idFor(c),providers:[...new Set([c.from,c.to])]})
}

import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {denyOwnedRead} from './batch.mjs'
import {beginPaste,waitActivityGone,endTransferObservation} from './progress.mjs'
import {cancelHeld,assertCancelledOutput,transferListeners} from './cancellation.mjs'
import {recoveryEvidence} from './recovery.mjs'

const definitions=[
  ...['local','usb'].flatMap(to=>['before','middle','denied'].map(point=>({from:'local',to,point}))),
  {from:'local',to:'cloud',point:'before'},
  {from:'cloud',to:'local',point:'before'},
  {from:'local',to:'cloud',point:'denied'},
]
export const overwriteCases=plan=>definitions.filter(c=>[c.from,c.to].every(k=>plan.targets.some(t=>t.kind===k)))
const idFor=c=>`overwrite-${c.from}-${c.to}-${c.point}`
export const overwriteManifest=plan=>overwriteCases(plan).map(c=>({id:idFor(c),name:`Overwrite recovery: ${idFor(c)}`,
  providers:[...new Set([c.from,c.to])],partIds:['outcome']}))
export function overwriteLocations(plan,c) {
  const id=idFor(c),from=child(plan.targets.find(t=>t.kind===c.from).files,`${id}-source`),
    to=child(plan.targets.find(t=>t.kind===c.to).files,`${id}-target`)
  return {id,from,to,source:child(from,'body.txt'),target:child(to,'body.txt')}
}
export const overwriteProbes=plan=>overwriteCases(plan).filter(c=>c.point!=='denied').map(c=>{
  const p=overwriteLocations(plan,c)
  return {id:p.id,source:p.source,target:p.target,holdPhase:c.point==='middle'?'written'
    :[c.from,c.to].includes('cloud')?'validation':'start',holdBytes:c.point==='middle'?16384:0,holdMs:5000,slowMs:0}
})
export async function overwrites(plan,fixture,ui,record) {
  for(const c of overwriteCases(plan)) await record(`Overwrite recovery: ${idFor(c)}`,async result=>{
    await ui.setView('list')
    await recordPart(result,'outcome',async part=>{
      result.phase='setup'
      const p=overwriteLocations(plan,c),content=`${p.id}/new\n`.padEnd(c.point==='denied'?4096:65536,'n'),original=`${p.id}/original\n`.padEnd(8192,'o'),
        source=new Map([['body.txt',content],['sentinel.txt','unrelated source\n']]),
        target=new Map([['body.txt',original],['sentinel.txt','unrelated target\n']])
      await fixture.mkdir(p.from);await fixture.mkdir(p.to)
      for(const [name,bytes] of source) await fixture.write(child(p.from,name),bytes)
      for(const [name,bytes] of target) await fixture.write(child(p.to,name),bytes)
      await ui.populateClipboard(p.from,[p.source],false,false)
      const restore=c.point==='denied'?await denyOwnedRead(fixture.roots,p.source):null
      try {
        result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to,{conflict:'Overwrite'})
        if(c.point==='denied') {
          part.feedback=await ui.expectedToast(/Paste failed:.*(?:denied|permission)/i,600_000)
          await waitActivityGone(ui);part.observations=await endTransferObservation(ui)
        } else Object.assign(part,await cancelHeld(ui,plan,p.id))
        part.ui='ACKNOWLEDGED'
      } finally {if(restore) await restore()}
      result.phase='verification'
      if(c.point==='middle') {
        const actual=await fixture.read(p.target);assertCancelledOutput(content,actual);target.set('body.txt',actual)
        assert.match(part.feedback,/rollback|recover|backup/i)
        assert.match(part.feedback,/Unknown completed count/)
        part.recovery=await recoveryEvidence(plan,original)
        assert.equal(part.recovery.originalCopies,1,'The exact old target must survive in private recovery')
        assert.ok(part.recovery.recoveryMarkers>0,'Retained original needs an explicit recovery marker')
        part.destination='INCOMPLETE_OUTPUT_WITH_PROTECTED_ORIGINAL'
      } else {
        part.recovery=await recoveryEvidence(plan,original)
        assert.match(part.feedback,/0 completed/);part.destination='ORIGINAL_TARGET_PRESERVED'
      }
      await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      assert.equal((await ui.handshake(plan.runId)).cancelTasks,0)
      assert.deepEqual(await transferListeners(ui),[])
      part.verification='INDEPENDENT_MATCH'
    })
  },{id:idFor(c),providers:[...new Set([c.from,c.to])]})
}

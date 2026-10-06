import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {assertCancelledOutput,transferListeners} from './cancellation.mjs'

export const ioFaultCases=plan=>[
  {from:'local',to:'local',point:'full-before',phase:'start',fault:'no-space'},
  {from:'local',to:'usb',point:'full-middle',phase:'written',fault:'no-space'},
  {from:'local',to:'local',point:'transient-middle',phase:'written',fault:'transient'},
  {from:'local',to:'cloud',point:'unavailable',phase:'start',fault:'unavailable'},
  {from:'cloud',to:'local',point:'unavailable',phase:'start',fault:'unavailable'},
  ...['local','cloud'].flatMap(from=>['copy','cut'].map(mode=>({from,to:'cloud',point:`preparing-${mode}`,phase:'prepare',mode}))),
].filter(c=>[c.from,c.to].every(kind=>plan.targets.some(t=>t.kind===kind)))
const idFor=c=>`iofaults-${c.from}-${c.to}-${c.point}`
export const ioFaultManifest=plan=>ioFaultCases(plan).map(c=>({id:idFor(c),
  name:`Owned I/O fault: ${c.from} to ${c.to}, ${c.point}`,providers:[...new Set([c.from,c.to])],partIds:['outcome']}))
export function ioFaultLocations(plan,c) {
  const id=idFor(c),from=child(plan.targets.find(t=>t.kind===c.from).files,`${id}-source`),
    to=child(plan.targets.find(t=>t.kind===c.to).files,`${id}-target`),name='body.txt'
  return {id,from,to,name,source:child(from,name),target:child(to,name)}
}
export const ioFaultProbes=plan=>ioFaultCases(plan).map(c=>{
  const p=ioFaultLocations(plan,c)
  return {id:p.id,source:p.source,target:p.target,holdPhase:c.phase,holdBytes:c.phase==='written'?16384:0,
    holdMs:c.phase==='prepare'?5000:0,slowMs:0,...(c.fault?{fault:c.fault}:{})}
})
export async function ioFaults(plan,fixture,ui,record) {
  for(const c of ioFaultCases(plan)) await record(`Owned I/O fault: ${c.from} to ${c.to}, ${c.point}`,async result=>{
    await ui.setView('list')
    await recordPart(result,'outcome',async part=>{
      result.phase='setup'
      const p=ioFaultLocations(plan,c),content=`${p.id}\n`.padEnd(65536,'x'),
        source=new Map([[p.name,content],['sentinel.txt','unrelated source\n']]),target=new Map([['sentinel.txt','unrelated target\n']])
      await fixture.mkdir(p.from);await fixture.mkdir(p.to)
      for(const [name,bytes] of source) await fixture.write(child(p.from,name),bytes)
      for(const [name,bytes] of target) await fixture.write(child(p.to,name),bytes)
      await ui.populateClipboard(p.from,[p.source],c.mode==='cut',false)
      result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to)
      if(c.phase==='prepare') {
        let status
        await ui.browser.waitUntil(async()=>{
          status=await ui.handshake(plan.runId)
          const probe=status.probes.find(p=>p.id===idFor(c));return probe?.consumed&&probe.observation.held
        },{timeout:180_000,interval:50,timeoutMsg:'Cloud metadata never reached its owned preparation delay'})
        assert.equal(status.cancelTasks,1,'Metadata preparation must own its cancellable read token before any mutation task')
        assert.equal((await transferListeners(ui)).length,1)
        await (await ui.browser.$('[aria-label="Cancel task"]')).click()
      }
      part.feedback=await ui.expectedToast(c.phase==='prepare'?/Paste failed:.*cancel/i:/Paste failed:.*(?:space|unavailable|transient)/i,600_000)
      assert.match(part.feedback,c.phase==='prepare'?/0 completed, 0 skipped, 0 failed, 1 not attempted/:/0 completed, 0 skipped, 1 failed, 0 not attempted/)
      await waitActivityGone(ui);part.observations=await endTransferObservation(ui);part.ui='ACKNOWLEDGED'
      if(c.phase==='prepare') assert.ok(part.observations.some(s=>s.label==='Cancelling…'))
      result.phase='verification'
      if(c.phase==='written') {
        const bytes=await fixture.read(p.target);assert.equal(bytes.length,16384);assertCancelledOutput(content,bytes)
        target.set(p.name,bytes);assert.match(part.feedback,/retained.*incomplete/i);part.destination='RETAINED_PREFIX'
      } else part.destination='NO_NEW_ENTRY'
      for(let i=0;i<2;i++) {
        if(i) await new Promise(resolve=>setTimeout(resolve,500))
        await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      }
      const status=await ui.handshake(plan.runId),probe=status.probes.find(probe=>probe.id===p.id)
      assert.ok(probe?.consumed);assert.equal(probe.faultUses,c.fault?1:0)
      assert.equal(probe.observation.checkpoints,c.phase==='prepare'?1:c.phase==='written'||[c.from,c.to].includes('cloud')?2:1,
        'No automatic transfer dispatch retry after the owned fault')
      assert.equal(status.cancelTasks,0);assert.deepEqual(await transferListeners(ui),[])
      if(c.mode==='cut') await ui.cutClipboard(p.from,[p.source])
      part.checkpoint=probe;part.verification='INDEPENDENT_MATCH';part.writesStopped=true
    })
  },{id:idFor(c),providers:[...new Set([c.from,c.to])]})
}

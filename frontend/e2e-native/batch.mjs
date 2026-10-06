import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import { constants } from 'node:fs'
import { child, noLinks, ownedPath } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'

const definitions = [
  { id: 'batch-local-copy', from: 'local', to: 'local', move: false, denied: true, rollback: true },
  { id: 'batch-usb-move', from: 'local', to: 'usb', move: true, denied: true, rollback: true },
  { id: 'batch-upload-copy', from: 'local', to: 'cloud', move: false, denied: true },
  { id: 'batch-upload-move', from: 'local', to: 'cloud', move: true, denied: true },
  { id: 'batch-cloud-copy', from: 'cloud', to: 'cloud', move: false, sourceFault: true },
  { id: 'batch-cloud-move', from: 'cloud', to: 'cloud', move: true, sourceFault: true },
  { id: 'batch-refresh-success', from: 'local', to: 'local', move: false, refreshFault: true, success: true },
  { id: 'batch-refresh-after-failure', from: 'local', to: 'local', move: false, denied: true, rollback: true, refreshFault: true },
]
const cases = plan => definitions.filter(c => [c.from,c.to].every(kind=>plan.targets.some(t=>t.kind===kind)))
const locations = (plan,c) => ({ from: child(plan.targets.find(t=>t.kind===c.from).files, `${c.id}-source`),
  to: child(plan.targets.find(t=>t.kind===c.to).files,`${c.id}-target`) })
export const batchManifest = plan => cases(plan).map(c=>({ id:c.id,name:`Batch failure and reconciliation: ${c.id}`,
  providers:[...new Set([c.from,c.to])],partIds:['outcome'] }))
export const batchFaults = plan => cases(plan).filter(c=>c.sourceFault||c.refreshFault).map(c=>{
  const {from,to}=locations(plan,c)
  return {id:c.id,operation:c.move?'move':'copy',...(c.sourceFault?{source:child(from,'b-failed.txt')}
    :{refreshTarget:to,armSource:child(from,'a-good.txt')})}
})
export async function denyOwnedRead(roots,raw,filesystem=fs) {
  ownedPath(roots,raw);assert.ok(!raw.startsWith('rclone://'));await noLinks(raw,filesystem)
  const handle=await filesystem.open(raw,constants.O_RDONLY|constants.O_NOFOLLOW)
  let stat, denied=false
  try { stat=await handle.stat();assert.ok(stat.isFile()&&stat.uid===process.getuid()&&stat.nlink===1&&stat.size<=4096)
    assert.equal(stat.mode&0o7777,0o600);await handle.chmod(0o000);denied=true
    let probe
    try {probe=await filesystem.open(raw,constants.O_RDONLY|constants.O_NOFOLLOW)}
    catch(error) {if(!['EACCES','EPERM'].includes(error.code)) throw error}
    if(probe){await probe.close();throw Error('Generated read denial is ineffective for this process')}
  } catch(error) {try {if(denied) await handle.chmod(0o600)}finally{await handle.close()}throw error}
  return async()=>{
    try {const current=await filesystem.lstat(raw);assert.ok(current.dev===stat.dev&&current.ino===stat.ino,'Denied fixture inode changed')
      await handle.chmod(0o600)
    } finally {await handle.close()}
  }
}
export async function batches(plan,fixture,ui,record) {
  for(const c of cases(plan)) await record(`Batch failure and reconciliation: ${c.id}`,async result=>{
    await ui.setView('list')
    await recordPart(result,'outcome',async part=>{
      result.phase='setup'
      const {from,to}=locations(plan,c),source=new Map(),target=new Map()
      await fixture.mkdir(from);await fixture.mkdir(to)
      const names=c.success?['a-good.txt']:['a-good.txt','b-failed.txt','c-after.txt','d-skipped.txt']
      for(const name of [...names,'unrelated-source.txt']) {
        const bytes=`${c.id}/${name}\nsource bytes\n`;await fixture.write(child(from,name),bytes);source.set(name,bytes)
      }
      for(const name of [...(c.success?[]:['d-skipped.txt']),'unrelated-target.txt']) {
        const bytes=`${c.id}/${name}\noriginal destination\n`;await fixture.write(child(to,name),bytes);target.set(name,bytes)
      }
      const restore=c.denied?await denyOwnedRead(fixture.roots,child(from,'b-failed.txt')):null
      let message
      try {
        result.phase='ui';part.ui='STARTED'
        await ui.populateClipboard(from,names.map(name=>child(from,name)),c.move,true)
        const counts=c.rollback?/0 completed, 1 skipped, 1 failed, 1 not attempted, 1 rolled back/
          :/1 completed, 1 skipped, 1 failed, 1 not attempted/
        const reason=c.sourceFault?/Injected owned source failure/:/denied|permission|operation failed/i
        const pattern=c.success?/Paste completed, but refresh failed/:/Paste failed:/
        message=await ui.paste(to,child(to,'a-good.txt'),{menu:true,...(!c.success?{conflict:'Skip'}:{}),expectedError:pattern,
          expectedRefresh:!!c.refreshFault})
        if(!c.success) {assert.match(message,counts);assert.match(message,reason)}
        if(c.refreshFault&&!c.success) assert.match(message,/Refresh also failed/)
        part.feedback=message;part.readFault=c.denied?'generated-local-file-mode-000':null
        part.ui='ACKNOWLEDGED';part.outcome=c.success?'SUCCESS_WITH_REFRESH_WARNING':'EXPECTED_BATCH_FAILURE'
        part.counts=c.success?{completed:1,skipped:0,failed:0,notAttempted:0}:{completed:c.rollback?0:1,skipped:1,failed:1,notAttempted:1,...(c.rollback?{rolledBack:1}:{})}
      } finally {if(restore) await restore()}
      result.phase='verification'
      if(c.success||!c.rollback){target.set('a-good.txt',source.get('a-good.txt'));if(c.move) source.delete('a-good.txt')}
      await verifyTree(fixture,from,source);await verifyTree(fixture,to,target)
      if(c.move) await ui.cutClipboard(from,names.filter(n=>source.has(n)).map(n=>child(from,n)))
      if(c.sourceFault||c.refreshFault){const status=await ui.handshake(plan.runId),fault=status.faults.find(f=>f.id===c.id)
        assert.equal(fault?.uses,1,'Declared native fault must be consumed exactly once');part.fault={id:c.id,uses:fault.uses}}
    })
  },{id:c.id,providers:[...new Set([c.from,c.to])]})
}

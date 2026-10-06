import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {constants} from 'node:fs'
import {child,ownedPath,noLinks} from './scope.mjs'
import {verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {heldProbe,transferListeners} from './cancellation.mjs'
import {recoveryEvidence} from './recovery.mjs'

export const raceCases=plan=>[
  {to:'local',point:'edit-copy',phase:'written'},
  {to:'usb',point:'edit-move',phase:'written',cut:true},
  {to:'local',point:'remove-copy',phase:'written'},
  {to:'local',point:'symlink-copy',phase:'written'},
  {to:'local',point:'late-collision',phase:'start',overwrite:true},
  {to:'cloud',point:'edit-upload-move',phase:'finalize',cut:true},
].filter(c=>plan.targets.some(t=>t.kind===c.to))
const idFor=c=>`races-local-${c.to}-${c.point}`
export const raceManifest=plan=>raceCases(plan).map(c=>({id:idFor(c),name:`Concurrent owned writer: ${idFor(c)}`,
  providers:[...new Set(['local',c.to])],partIds:['outcome']}))
export function raceLocations(plan,c) {
  const id=idFor(c),from=child(plan.targets.find(t=>t.kind==='local').files,`${id}-source`),
    to=child(plan.targets.find(t=>t.kind===c.to).files,`${id}-target`)
  return {id,from,to,source:child(from,'body.txt'),target:child(to,'body.txt')}
}
export const raceProbes=plan=>raceCases(plan).map(c=>{
  const p=raceLocations(plan,c)
  return {id:p.id,source:p.source,target:p.target,holdPhase:c.phase,holdBytes:c.phase==='written'?16384:0,holdMs:5000,slowMs:0}
})
export async function ownedWriter(roots,raw,{filesystem=fs}={}) {
  ownedPath(roots,raw);assert.ok(!roots.includes(raw)&&!raw.startsWith('rclone://'));await noLinks(raw,filesystem)
  const handle=await filesystem.open(raw,constants.O_RDWR|constants.O_NOFOLLOW)
  try {
    const stat=await handle.stat()
    assert.ok(stat.isFile()&&stat.uid===process.getuid()&&stat.nlink===1&&stat.size<=65536&&(stat.mode&0o777)===0o600)
    const same=async(path=raw)=>{const current=await filesystem.lstat(path);assert.ok(current.isFile()&&current.dev===stat.dev&&current.ino===stat.ino,'Owned writer inode changed')}
    return {handle,same,stat,write:async bytes=>{
      assert.ok(typeof bytes==='string'&&Buffer.byteLength(bytes)<=65536);await same()
      const written=await handle.write(bytes,0,'utf8');assert.equal(written.bytesWritten,Buffer.byteLength(bytes))
      await handle.truncate(Buffer.byteLength(bytes));await handle.sync();await same()
    }}
  } catch(error) {await handle.close();throw error}
}
export async function ownedSymlinkSwap(roots,raw,referent) {
  ownedPath(roots,referent);assert.notEqual(raw,referent);await noLinks(referent,fs)
  const referred=await fs.lstat(referent)
  assert.ok(referred.isFile()&&referred.uid===process.getuid()&&referred.nlink===1&&referred.size<=65536)
  const writer=await ownedWriter(roots,raw),saved=child(raw.slice(0,raw.lastIndexOf('/')),'source.saved')
  try {
    await writer.same();await assert.rejects(fs.lstat(saved),{code:'ENOENT'})
    await fs.rename(raw,saved);await fs.symlink(referent,raw)
    const link=await fs.lstat(raw);assert.ok(link.isSymbolicLink());await writer.same(saved)
    return async()=>{
      try {
        const current=await fs.lstat(raw)
        assert.ok(current.isSymbolicLink()&&current.dev===link.dev&&current.ino===link.ino,'Owned link changed; refuse replacement')
        assert.equal(await fs.readlink(raw),referent);await writer.same(saved)
        await fs.unlink(raw);await fs.rename(saved,raw);await writer.same()
      } finally {await writer.handle.close()}
    }
  } catch(error) {await writer.handle.close();throw error}
}
export async function races(plan,fixture,ui,record) {
  for(const c of raceCases(plan)) await record(`Concurrent owned writer: ${idFor(c)}`,async result=>{
    await ui.setView('list')
    await recordPart(result,'outcome',async part=>{
      result.phase='setup'
      const p=raceLocations(plan,c),content=`${p.id}/old\n`.padEnd(65536,'o'),changed=`${p.id}/changed\n`.padEnd(65536,'c'),
        original=`${p.id}/original\n`.padEnd(8192,'b'),foreign=`${p.id}/concurrent-target\n`.padEnd(4096,'f'),
        source=new Map([['body.txt',content],['sentinel.txt','unrelated source\n']]),target=new Map([['sentinel.txt','unrelated target\n']])
      if(c.overwrite) target.set('body.txt',original)
      await fixture.mkdir(p.from);await fixture.mkdir(p.to)
      for(const [name,bytes] of source) await fixture.write(child(p.from,name),bytes)
      for(const [name,bytes] of target) await fixture.write(child(p.to,name),bytes)
      await ui.populateClipboard(p.from,[p.source],!!c.cut,false)
      result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to,c.overwrite?{conflict:'Overwrite'}:{})
      part.checkpoint=await heldProbe(ui,plan.runId,p.id)
      let restore
      if(c.point==='symlink-copy') restore=await ownedSymlinkSwap(fixture.roots,p.source,child(p.from,'sentinel.txt'))
      else if(c.overwrite) await fixture.write(p.target,foreign)
      else {
        const writer=await ownedWriter(fixture.roots,p.source)
        try {
          if(c.point==='remove-copy') {await writer.same();await fs.unlink(p.source);source.delete('body.txt')}
          else {await writer.write(changed);source.set('body.txt',changed)}
        } finally {await writer.handle.close()}
      }
      try {
        part.feedback=await ui.expectedToast(/Paste failed:.*(?:changed|verify|symlink|exists|rollback|retained)/i,600_000)
        await waitActivityGone(ui);part.observations=await endTransferObservation(ui);part.ui='ACKNOWLEDGED'
      } finally {if(restore) {assert.equal((await ui.handshake(plan.runId)).cancelTasks,0,'Restore the owned link only after the transfer task has stopped');await restore()}}
      result.phase='verification'
      if(c.overwrite) {
        target.set('body.txt',foreign);part.recovery=await recoveryEvidence(plan,original)
        assert.equal(part.recovery.originalCopies,1);assert.ok(part.recovery.recoveryMarkers>0)
        assert.match(part.feedback,/Unknown completed count/)
      } else {
        target.set('body.txt',c.point==='edit-copy'||c.point==='edit-move'?content.slice(0,16384)+changed.slice(16384):content)
        assert.match(part.feedback,/retained|inspection|source.*not removed/i)
        assert.match(part.feedback,/0 completed/)
      }
      for(let i=0;i<2;i++) {
        if(i) await new Promise(resolve=>setTimeout(resolve,500))
        await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      }
      const status=await ui.handshake(plan.runId)
      assert.equal(status.cancelTasks,0);assert.deepEqual(await transferListeners(ui),[])
      if(c.cut) await ui.cutClipboard(p.from,[p.source])
      part.verification='INDEPENDENT_MATCH';part.writesStopped=true;part.concurrentMutation=c.point
    })
  },{id:idFor(c),providers:[...new Set(['local',c.to])]})
}

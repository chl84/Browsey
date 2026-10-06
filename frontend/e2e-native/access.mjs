import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {constants} from 'node:fs'
import {child,ownedPath,noLinks} from './scope.mjs'
import {editing,editingManifest,propertiesParts,verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {denyOwnedRead} from './batch.mjs'
import {beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {transferListeners} from './cancellation.mjs'

export const accessManifest=plan=>[...editingManifest(plan,'properties').map(c=>({...c,partIds:propertiesParts})),{id:'access-local',name:'Owned local access denials and read-only source',
  providers:['local'],partIds:['read-denied','write-denied','read-only-copy']}]
export async function ownedMode(roots,raw,mode,{directory=false,filesystem=fs}={}) {
  ownedPath(roots,raw);assert.ok(!raw.startsWith('rclone://'));await noLinks(raw,filesystem)
  assert.ok([0o400,0o500].includes(mode),'Only declared restrictive modes are allowed')
  const handle=await filesystem.open(raw,constants.O_RDONLY|constants.O_NOFOLLOW|(directory?constants.O_DIRECTORY:0))
  const stat=await handle.stat(),original=stat.mode&0o777
  try {
    assert.ok((directory?stat.isDirectory():stat.isFile())&&stat.uid===process.getuid())
    assert.equal(original,directory?0o700:0o600)
    await handle.chmod(mode);assert.equal((await handle.stat()).mode&0o777,mode,'This fixture must actually support the declared Unix restriction')
  } catch(error) {try {await handle.chmod(original)} finally {await handle.close()}throw error}
  return async()=>{
    try {const current=await filesystem.lstat(raw);assert.ok(current.dev===stat.dev&&current.ino===stat.ino,'Restricted fixture inode changed')
      await handle.chmod(original)
    } finally {await handle.close()}
  }
}
export async function access(plan,fixture,ui,record) {
  await editing(plan,fixture,ui,record,'properties')
  await record('Owned local access denials and read-only source',async result=>{
    await ui.setView('list')
    for(const point of ['read-denied','write-denied','read-only-copy']) await recordPart(result,point,async part=>{
      result.phase='setup'
      const root=plan.targets.find(t=>t.kind==='local').files,from=child(root,`access-${point}-source`),to=child(root,`access-${point}-target`),
        source=child(from,'body.txt'),target=child(to,'body.txt'),content=`access-${point}\nsource\n`,
        expectedSource=new Map([['body.txt',content],['sentinel.txt','unrelated source\n']]),expectedTarget=new Map([['sentinel.txt','unrelated target\n']])
      await fixture.mkdir(from);await fixture.mkdir(to)
      for(const [name,bytes] of expectedSource) await fixture.write(child(from,name),bytes)
      for(const [name,bytes] of expectedTarget) await fixture.write(child(to,name),bytes)
      await ui.populateClipboard(from,[source],false,false)
      const restore=point==='read-denied'?await denyOwnedRead(fixture.roots,source)
        :await ownedMode(fixture.roots,point==='write-denied'?to:source,point==='write-denied'?0o500:0o400,{directory:point==='write-denied'})
      try {
        result.phase='ui';part.ui='STARTED';await beginPaste(ui,to)
        if(point==='read-only-copy') {
          await ui.idle({resultPath:target});expectedTarget.set('body.txt',content)
          assert.equal((await fs.lstat(target)).mode&0o777,0o400,'Copy must preserve the supported read-only source mode')
          part.outcome='READ_ONLY_SOURCE_COPIED'
        } else {
          part.feedback=await ui.expectedToast(/Paste failed:.*(?:denied|permission)/i,600_000)
          assert.match(part.feedback,/0 completed, 0 skipped, 1 failed, 0 not attempted/)
          part.outcome='ACTIONABLE_ACCESS_DENIAL'
        }
        await waitActivityGone(ui);part.observations=await endTransferObservation(ui);part.ui='ACKNOWLEDGED'
      } finally {await restore()}
      result.phase='verification'
      await verifyTree(fixture,from,expectedSource);await verifyTree(fixture,to,expectedTarget)
      assert.equal((await ui.handshake(plan.runId)).cancelTasks,0);assert.deepEqual(await transferListeners(ui),[])
      part.verification='INDEPENDENT_MATCH'
    })
  },{id:'access-local',providers:['local']})
}

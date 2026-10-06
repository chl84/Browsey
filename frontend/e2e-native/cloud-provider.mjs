import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyTree} from './editing.mjs'
import {releasedResources} from './resources.mjs'
import {beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {workingCopies,editWorkingCopy,uploadEditedCopy,releasedWorkspace} from './cloud-workspaces.mjs'

const original='cloud version A\n',changed='cloud version B\n',edited='working edits C\n'
export function cloudLocations(plan) {
  const local=plan.targets.find(t=>t.kind==='local'),cloud=plan.targets.find(t=>t.kind==='cloud')
  const from=child(local.files,'provider-cloud-source'),to=child(cloud.files,'provider-cloud'),
    downloaded=child(local.files,'provider-cloud-download'),clone=child(cloud.files,'provider-cloud-clone'),
    conflict=child(local.files,'provider-cloud-conflict'),errors=child(local.files,'provider-cloud-errors')
  return {from,to,downloaded,clone,conflict,errors,source:child(to,'working.txt')}
}
export function cloudManifest(plan,scope='full') {
  assert.ok(['full','working'].includes(scope))
  assert.deepEqual(plan.targets.map(t=>t.kind),['local','cloud'],'Cloud acceptance requires only local and the approved cloud')
  return [{id:'provider-cloud',name:'Owned OneDrive refresh/transfers/conflicts/working copies',providers:['local','cloud'],
    partIds:['refresh','upload','cloud-copy','download','conflict-cancel','conflict-skip','conflict-rename','working-prepare','working-upload','conflict-overwrite','working-changed-source'].filter(id=>scope==='full'||!['cloud-copy','download','conflict-cancel','conflict-skip'].includes(id))},
  {id:'provider-cloud-errors',name:'Exact candidate cloud quota/rate/auth failures',providers:['local','cloud'],partIds:['quota','rate','auth']}]
}
export const cloudProbes=plan=>['quota','rate','auth'].map(kind=>{
  const p=cloudLocations(plan),name=`${kind}.txt`
  return {id:`provider-cloud-${kind}`,source:child(p.errors,name),target:child(p.to,name),holdPhase:'validation',holdBytes:0,holdMs:0,slowMs:0,
    fault:{quota:'cloud-quota',rate:'cloud-rate-limited',auth:'cloud-auth-required'}[kind]}
})
export async function cloudProvider(plan,fixture,ui,record,scope='full') {
  const manifest=cloudManifest(plan,scope);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  const p=cloudLocations(plan),expected=new Map([['sentinel.txt','cloud sentinel\n']])
  await record('Owned OneDrive refresh/transfers/conflicts/working copies',async result=>{
    result.phase='setup'
    for(const dir of [p.from,p.to,p.downloaded,p.clone,p.conflict,p.errors]) await fixture.mkdir(dir)
    await fixture.write(child(p.to,'sentinel.txt'),expected.get('sentinel.txt'))
    await fixture.write(child(p.from,'working.txt'),original);await fixture.write(child(p.conflict,'working.txt'),changed)
    const step=(id,action)=>manifest[0].partIds.includes(id)?recordPart(result,id,async part=>{result.phase='ui';part.ui='STARTED';const started=Date.now();await action(part);part.ui='ACKNOWLEDGED';part.elapsedMs=Date.now()-started;result.phase='verification';await verifyTree(fixture,p.to,expected);await releasedResources(ui,plan.runId);part.verification='INDEPENDENT_MATCH'}):Promise.resolve()
    await step('refresh',async()=>{await ui.navigate(p.to);await ui.setView('list');await ui.listing(p.to,'list',[child(p.to,'sentinel.txt')]);await fixture.write(child(p.to,'refresh.txt'),'refresh fixture\n');expected.set('refresh.txt','refresh fixture\n');await ui.refresh();await ui.listing(p.to,'list',[...expected.keys()].map(n=>child(p.to,n)))})
    await step('upload',async()=>{await ui.transfer(child(p.from,'working.txt'),p.to);expected.set('working.txt',original);assert.equal(await fixture.read(child(p.from,'working.txt')),original)})
    await step('cloud-copy',async()=>{await ui.transfer(p.source,p.clone);await verifyTree(fixture,p.clone,new Map([['working.txt',original]]))})
    await step('download',async()=>{await ui.transfer(p.source,p.downloaded);await verifyTree(fixture,p.downloaded,new Map([['working.txt',original]]))})
    for(const [id,choice] of [['cancel','Cancel'],['skip','Skip'],['rename','Auto-rename']]) await step(`conflict-${id}`,async()=>{
      await ui.populateClipboard(p.conflict,[child(p.conflict,'working.txt')],false,false)
      await ui.paste(p.to,child(p.to,id==='rename'?'working-1.txt':'working.txt'),{conflict:choice})
      if(id==='rename') expected.set('working-1.txt',changed)
      assert.equal(await fixture.read(child(p.conflict,'working.txt')),changed)
    })
    let copyId
    await step('working-prepare',async part=>{
      await ui.navigate(p.to);await ui.select(p.source);await ui.browser.keys(['Enter'])
      await ui.idle({toast:'Opened a local working copy. Upload edits from Settings → Cloud → Working copies.'},600_000)
      await releasedWorkspace(ui,plan.runId)
      const copies=await workingCopies(plan,p.source);assert.equal(copies.length,1);assert.deepEqual(copies[0].bytes,Buffer.from(original));copyId=copies[0].id
      await editWorkingCopy(plan,p.source,copyId,original,edited)
      part.externalEditor='DISABLED';part.autoUpload=false;part.privateCopy='INDEPENDENT_MATCH'
    })
    await step('working-upload',async part=>{
      const uploaded=await uploadEditedCopy(ui,p.source,{changed:false});expected.set(uploaded.split('/').at(-1),edited)
      await releasedWorkspace(ui,plan.runId);await ui.refresh();await ui.listing(p.to,'list',[...expected.keys()].map(n=>child(p.to,n)))
      assert.deepEqual((await workingCopies(plan,p.source)).find(c=>c.id===copyId).bytes,Buffer.from(edited));part.sourceChanged=false
    })
    await step('conflict-overwrite',async()=>{
      await ui.populateClipboard(p.conflict,[child(p.conflict,'working.txt')],false,false);await ui.paste(p.to,p.source,{conflict:'Overwrite'})
      expected.set('working.txt',changed);assert.equal(await fixture.read(child(p.conflict,'working.txt')),changed)
    })
    await step('working-changed-source',async part=>{
      const uploaded=await uploadEditedCopy(ui,p.source,{changed:true});assert.ok(!expected.has(uploaded.split('/').at(-1)));expected.set(uploaded.split('/').at(-1),edited)
      await releasedWorkspace(ui,plan.runId);await ui.refresh();await ui.listing(p.to,'list',[...expected.keys()].map(n=>child(p.to,n)))
      assert.deepEqual((await workingCopies(plan,p.source)).find(c=>c.id===copyId).bytes,Buffer.from(edited));part.sourceChanged=true
    })
  },{id:'provider-cloud',providers:['local','cloud']})
  await record('Exact candidate cloud quota/rate/auth failures',async result=>{
    for(const kind of ['quota','rate','auth']) await recordPart(result,kind,async part=>{
      result.phase='setup';const name=`${kind}.txt`,source=child(p.errors,name),bytes=`owned ${kind} source\n`;await fixture.write(source,bytes)
      await ui.populateClipboard(p.errors,[source],true,false);result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to)
      part.feedback=await ui.expectedToast({quota:/Paste failed:.*quota exceeded/i,rate:/Paste failed:.*rate limit/i,auth:/Paste failed:.*authentication failed/i}[kind],600_000)
      assert.match(part.feedback,/0 completed, 0 skipped, 1 failed, 0 not attempted/)
      await waitActivityGone(ui);part.progress=await endTransferObservation(ui);part.ui='ACKNOWLEDGED';result.phase='verification'
      assert.equal(await fixture.read(source),bytes);await verifyTree(fixture,p.to,expected);await releasedResources(ui,plan.runId)
      const probe=(await ui.handshake(plan.runId)).probes.find(p=>p.id===`provider-cloud-${kind}`)
      assert.ok(probe?.consumed);assert.equal(probe.faultUses,1);assert.equal(probe.observation.checkpoints,1)
      assert.equal(probe.observation.faultCode,kind==='auth'?'auth_required':'rate_limited')
      await ui.cutClipboard(p.errors,[source]);part.checkpoint=probe;part.verification='INDEPENDENT_PRESERVATION';part.injection='CANDIDATE_ONLY_PREWRITE'
    })
  },{id:'provider-cloud-errors',providers:['local','cloud']})
}

export const cloudWorking=(plan,fixture,ui,record)=>cloudProvider(plan,fixture,ui,record,'working')

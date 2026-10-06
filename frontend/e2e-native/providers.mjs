import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {child,noLinks} from './scope.mjs'
import {networkDeleteFixture,undoMetadata} from './provider-fixtures.mjs'
import {recordPart} from './report.mjs'
import {progress,progressManifest,progressProbes} from './progress.mjs'
import {cancellations,cancellationManifest,cancellationProbes} from './cancellation.mjs'
import {releasedResources} from './resources.mjs'
import {foundation,foundationManifest} from './cases.mjs'
import {access,accessManifest} from './access.mjs'

export function usbManifest(plan) {
  assert.deepEqual(plan.targets.map(t=>t.kind),['local','usb'],'USB acceptance requires only local and USB')
  return [...foundationManifest(plan),...accessManifest(plan,'usb')]
}
export async function usb(plan,fixture,ui,record) {
  usbManifest(plan)
  ui.waitTimeout=180_000;ui.transferTimeout=600_000
  await foundation(plan,fixture,ui,record)
  await access(plan,fixture,ui,record,'usb')
}

export function networkManifest(plan) {
  assert.deepEqual(plan.targets.map(t=>t.kind),['local','network'],'Network acceptance requires only local and network')
  return [...foundationManifest(plan),...progressManifest(plan),...cancellationManifest(plan),
    {id:'provider-network',name:'Owned SFTP/FUSE deletion and stale paths',providers:['network'],
      partIds:['large-delete-cancel','large-delete-confirm','stale-rename','fresh-metadata']}]
}
export async function network(plan,fixture,ui,record) {
  networkManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  await foundation(plan,fixture,ui,record)
  await progress(plan,fixture,ui,record)
  await cancellations(plan,fixture,ui,record)
  await record('Owned SFTP/FUSE deletion and stale paths',async result=>{
    const target=plan.targets.find(t=>t.kind==='network'),base=child(target.files,'provider-network'),large=child(base,'large-delete.bin'),sentinel=child(base,'sentinel.txt')
    result.phase='setup';await fixture.mkdir(base);await fixture.write(sentinel,'network sentinel\n')
    result.transport=target.path.includes('/sftp:')?'SFTP via GVFS/FUSE':'configured network filesystem'
    result.largeFixture=await networkDeleteFixture(plan,large)
    const before=await undoMetadata(plan)
    for(const cancel of [true,false]) await recordPart(result,cancel?'large-delete-cancel':'large-delete-confirm',async part=>{
      result.phase='ui';part.ui='STARTED';const start=Date.now();await ui.remove(large,cancel);part.elapsedMs=Date.now()-start;part.ui='ACKNOWLEDGED'
      result.phase='verification';assert.equal(await fixture.exists(large),cancel);assert.equal(await fixture.read(sentinel),'network sentinel\n')
      if(cancel) assert.equal((await fs.lstat(large)).size,result.largeFixture.bytes)
      assert.deepEqual(await undoMetadata(plan),before,'Network deletion must create no local undo content backup')
      part.verification='INDEPENDENT_METADATA_MATCH';part.localContentBackup=false
    })
    const stale=child(base,'stale.txt'),moved=child(base,'externally-renamed.txt')
    await recordPart(result,'stale-rename',async part=>{
      result.phase='setup';await fixture.write(stale,'preserved stale source\n')
      result.phase='ui';await ui.navigate(base);await ui.setView('list');await ui.beginRename(stale)
      await noLinks(stale,fs);await fs.rename(stale,moved)
      part.ui='STARTED';await ui.renameDraft('must-not-appear.txt',{error:/not found|no such file|does not exist/i});await ui.cancelRename();part.ui='ACKNOWLEDGED'
      result.phase='verification';assert.equal(await fixture.read(moved),'preserved stale source\n');assert.equal(await fixture.exists(child(base,'must-not-appear.txt')),false)
      part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'fresh-metadata',async part=>{
      result.phase='ui';part.ui='STARTED';await ui.refresh();await ui.listing(base,'list',[sentinel,moved]);part.ui='ACKNOWLEDGED'
      result.phase='verification';assert.deepEqual((await fixture.snapshot(base)).map(x=>x.path).sort(),[sentinel,moved].sort());await releasedResources(ui,plan.runId)
      part.verification='FUSE_UI_MEMBERSHIP_MATCH'
    })
  },{id:'provider-network',providers:['network']})
}

export const networkProbes=plan=>[...progressProbes(plan),...cancellationProbes(plan)]

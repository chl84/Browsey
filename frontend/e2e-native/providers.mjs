import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {child,noLinks} from './scope.mjs'
import {networkDeleteFixture,undoMetadata} from './provider-fixtures.mjs'
import {recordPart} from './report.mjs'
import {progress,progressManifest,progressProbes} from './progress.mjs'
import {cancellations,cancellationManifest,cancellationProbes} from './cancellation.mjs'
import {names,namesManifest} from './names.mjs'
import {providerImages} from './provider-images.mjs'
import {verifyByteTree} from './byte-tree.mjs'
import {startThumbnailObservation,finishThumbnailObservation,assertThumbnailSnapshots,waitThumbnails} from './mobile-thumbnails.mjs'
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

const mobileOnly=plan=>({...plan,targets:plan.targets.filter(t=>t.kind==='mobile')})
export function mobileManifest(plan) {
  assert.deepEqual(plan.targets.map(t=>t.kind),['local','mobile'],'MTP acceptance requires only local and mobile')
  return [...foundationManifest(plan),...namesManifest(mobileOnly(plan)),{id:'provider-mobile',
    name:'Owned MTP metadata and thumbnails',providers:['mobile'],partIds:['cold-grid','late-grid','list-metadata','preserved-images']}]
}
export async function mobile(plan,fixture,ui,record) {
  mobileManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  await foundation(plan,fixture,ui,record)
  await names(mobileOnly(plan),fixture,ui,record)
  await record('Owned MTP metadata and thumbnails',async result=>{
    const target=plan.targets.find(t=>t.kind==='mobile'),base=child(target.files,'provider-mobile'),
      expected=new Map([['01-red.png',providerImages[0]],['02-blue.png',providerImages[1]],['sentinel.txt',Buffer.from('MTP image sentinel\n')]])
    result.phase='setup';await fixture.mkdir(base)
    for(const [name,bytes] of expected) await fixture.write(child(base,name),bytes)
    await ui.navigate(base);await ui.setView('list');await ui.sort('Name','asc')
    for(const late of [false,true]) await recordPart(result,late?'late-grid':'cold-grid',async part=>{
      result.phase='setup'
      if(late) {await fixture.write(child(base,'03-late.png'),providerImages[0]);expected.set('03-late.png',providerImages[0])}
      const paths=[...expected.keys()].sort().map(n=>child(base,n)),images=paths.filter(p=>p.endsWith('.png'))
      result.phase='ui';part.ui='STARTED';const started=Date.now();await startThumbnailObservation(ui)
      try {
        if(late) await ui.refresh();else await ui.setView('grid')
        await ui.listing(base,'grid',paths,{fileOrder:paths});await waitThumbnails(ui,images)
      } catch(error) {part.observations=await finishThumbnailObservation(ui);throw error}
      part.observations=await finishThumbnailObservation(ui);part.elapsedMs=Date.now()-started;part.ui='ACKNOWLEDGED'
      result.phase='verification';assertThumbnailSnapshots(part.observations,paths,images);part.verification='DECODED_THUMBNAILS_STABLE_ORDER'
    })
    await recordPart(result,'list-metadata',async part=>{
      result.phase='ui';part.ui='STARTED';await ui.setView('list');await ui.refresh()
      const paths=[...expected.keys()].sort().map(n=>child(base,n));await ui.listing(base,'list',paths,{fileOrder:paths})
      const late=child(base,'03-late.png');await ui.propertiesOpen(base,[late]);let rows
      await ui.browser.waitUntil(async()=>{rows=await ui.propertiesRows();return rows.Size?.includes(`${providerImages[0].length} B`)},{timeout:60_000,interval:100,timeoutMsg:'MTP properties size did not finish'})
      assert.equal(rows.Type,'file');assert.equal(rows.Name,'03-late.png');assert.ok(rows.Size.includes(`${providerImages[0].length} B`))
      await ui.closeProperties(base);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_SIZE_NAME_TYPE'
    })
    await recordPart(result,'preserved-images',async part=>{
      result.phase='verification';part.tree=await verifyByteTree(fixture,base,expected);await releasedResources(ui,plan.runId)
      part.verification='INDEPENDENT_BINARY_DIGESTS';part.ui='NOT_SENT'
    })
  },{id:'provider-mobile',providers:['mobile']})
}

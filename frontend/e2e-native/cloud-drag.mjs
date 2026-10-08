/* global document */
import assert from 'node:assert/strict'
import {child,cloudCanonicalPath,ownedPath} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyByteTree} from './byte-tree.mjs'
import {releasedResources} from './resources.mjs'
import {ownedWindow,nativeInput,observeDrag,endDragObservation,dragPoints,elementPoint} from './drag.mjs'

const parts=['onedrive-to-drive','drive-to-onedrive','cancel','conflict-cancel']
export const cloudDragManifest=plan=>{
  assert.deepEqual(plan.targets.map(t=>t.kind),['local','cloud','cloud'])
  assert.deepEqual(new Set(plan.targets.slice(1).map(t=>t.path.slice(9).split('/')[0])),new Set(['Onedrive','Google Disk']))
  return [{id:'desktop-cloud-drag',name:'Real cross-instance OneDrive and Google Drive copies',providers:['local','cloud'],partIds:parts}]
}

async function actualPath(ui,raw) {
  ownedPath(ui.roots,raw)
  const paths=await ui.browser.execute(()=>[...document.querySelectorAll('[data-path]')].filter(n=>n.getClientRects().length).map(n=>n.dataset.path))
  const match=paths.find(p=>cloudCanonicalPath(p)===raw);assert.ok(match,'Generated fixture entry must be visible');ownedPath(ui.roots,match);return match
}
async function navigate(ui,raw) {
  ownedPath(ui.roots,raw);await ui.idle();await ui.browser.releaseActions()
  const root=ui.roots.find(root=>raw===root||raw.startsWith(root+'/'))
  const waitPath=async expected=>{
    let failure
    await ui.browser.waitUntil(async()=>{
      const state=await ui.browser.execute(()=>({current:document.querySelector('main.shell')?.dataset.currentPath,
        errors:[...document.querySelectorAll('.toast[role="status"],.notice-error,.pill.error')].filter(n=>n.getClientRects().length).map(n=>n.textContent.trim())}))
      failure=state.errors.find(e=>/failed|could not|not permitted|unregistered/i.test(e))
      return !!failure || (state.current && cloudCanonicalPath(state.current)===cloudCanonicalPath(expected))
    },{timeout:60000,interval:150,timeoutMsg:'Cloud fixture navigation did not reach its canonical path'})
    assert.ok(!failure,failure);await ui.idle()
  }
  const bookmark=await ui.browser.$(`.bookmark[data-drop-path=${JSON.stringify(root)}]`);await bookmark.waitForDisplayed({timeout:10000});await bookmark.click();await waitPath(root);await ui.refresh()
  let current=root
  for(const part of raw.slice(root.length+1).split('/').filter(Boolean)) {
    current=child(current,part);const actual=await actualPath(ui,current);await ui.select(actual);await ui.browser.keys(['Enter']);await waitPath(actual)
  }
}

export async function cloudDrag(plan,fixture,ui,record) {
  cloudDragManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  const drive=plan.targets.find(t=>t.path.startsWith('rclone://Google Disk/')),one=plan.targets.find(t=>t.path.startsWith('rclone://Onedrive/'))
  const names=['first æ #?%+ spaced.txt','second spaced.txt'],expected=new Map(names.map((name,i)=>[name,Buffer.from(`cross-instance cloud fixture ${i}\n`)]))
  const tree='folder æ #',locations=new Map()
  expected.set(tree,null);expected.set(tree+'/nested.txt',Buffer.from('nested cloud folder contents\n'));expected.set(tree+'/empty',null)
  for(const id of parts) {
    const from=child((id==='onedrive-to-drive'?one:drive).files,id+'-source'),to=child((id==='onedrive-to-drive'?drive:one).files,id+'-target')
    await fixture.mkdir(from);await fixture.mkdir(to)
    await fixture.uploadCloudDragTree(from,expected)
    if(id==='conflict-cancel')await fixture.write(child(to,names[0]),Buffer.from('destination retained\n'))
    locations.set(id,{from,to})
  }
  await fixture.registerCloudDragIds()
  await record('Real cross-instance OneDrive and Google Drive copies',async result=>{
    await ui.withPeer(async peer=>{
      peer.waitTimeout=180_000;peer.transferTimeout=600_000
      for(const id of parts)await recordPart(result,id,async part=>{
        result.phase='ui';part.ui='STARTED'
        const {from,to}=locations.get(id)
        await navigate(ui,from);await ui.setView(id==='drive-to-onedrive'?'grid':'list');await ownedWindow(ui,[0,0,940,950])
        await navigate(peer,to);await peer.setView('list');const target=await ownedWindow(peer,[1000,0,940,950])
        const first=await actualPath(ui,child(from,names[0])),second=await actualPath(ui,child(from,names[1]))
        await ui.select(first);await ui.modifiedSelect(second,'Control');await ui.modifiedSelect(await actualPath(ui,child(from,tree)),'Control');await observeDrag(ui);await observeDrag(peer)
        try {
          part.gesture=await nativeInput(ui,'drag',{points:dragPoints(await elementPoint(ui,first),[target.origin[0]+650,target.origin[1]+650],25),mode:id==='drive-to-onedrive'?'move':'default',modifier_after_press:true,cancel:id==='cancel'})
          if(id==='conflict-cancel') {
            const modal=await peer.browser.$('[role="dialog"]');await modal.waitForDisplayed({timeout:180_000});await(await modal.$('button=Cancel')).click()
          } else if(id!=='cancel') {
            await peer.browser.waitUntil(()=>fixture.exists(child(to,names[0])),{timeout:180_000,interval:300,timeoutMsg:'No independently observed cross-cloud copy'})
          }
          await peer.idle();await ui.idle()
        } finally {part.sourceEvents=await endDragObservation(ui);part.targetEvents=await endDragObservation(peer)}
        assert.ok(part.sourceEvents.events.some(e=>e.type==='dragstart'&&e.trusted),'Trusted native source drag required')
        if(id!=='cancel')assert.ok(part.targetEvents.events.some(e=>e.type==='tauri://drag-drop'),'Native receiving drop required')
        part.source=await verifyByteTree(fixture,from,expected)
        part.target=await verifyByteTree(fixture,to,id==='cancel'?new Map():id==='conflict-cancel'?new Map([[names[0],Buffer.from('destination retained\n')]]):expected)
        await releasedResources(peer,plan.runId);await releasedResources(ui,plan.runId)
        part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_SOURCE_AND_DESTINATION_BYTES';part.policy='copy only, including Shift';part.originalsDeleted=false
      })
    })
  },{id:'desktop-cloud-drag',providers:['local','cloud']})
}

/* global document */
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {child,noLinks} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyByteTree} from './byte-tree.mjs'
import {regularFile} from './fixtures.mjs'
import {releasedResources} from './resources.mjs'
import {ownedWindow,nativeInput,observeDrag,endDragObservation,dragPoints} from './drag.mjs'

const names=['first æ #?%+ spaced.txt','second spaced.txt']
const expected=new Map(names.map((name,i)=>[name,Buffer.from(`generated cloud export ${i}\n`)]))
export const cloudExportManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local','cloud']);return [{id:'desktop-cloud-export',name:'Prepared OneDrive copies to isolated Nautilus',providers:['local','cloud'],partIds:['prepare','cancel-drag','receiver-copy','preserved-cloud-and-staging']}]}
export async function cloudExport(plan,fixture,ui,record) {
  cloudExportManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  const local=plan.targets[0],cloud=plan.targets[1],source=child(cloud.files,'cloud-export-source'),receiver=child(local.files,'cloud-export-receiver')
  await record('Prepared OneDrive copies to isolated Nautilus',async result=>{
    result.phase='setup';await fixture.mkdir(source);await fixture.mkdir(receiver)
    for(const [name,bytes]of expected)await fixture.write(child(source,name),bytes)
    await ui.navigate(source);await ui.setView('list');await ownedWindow(ui,[0,0,940,950]);await ui.select(child(source,names[0]));await ui.modifiedSelect(child(source,names[1]),'Control')
    await ui.selection(source,names.map(n=>child(source,n)))
    const modal=()=>ui.browser.$('[role="dialog"]')
    await recordPart(result,'prepare',async part=>{
      result.phase='ui';part.ui='STARTED';await ui.menuAction('cloud-export',child(source,names[0]));await(await modal()).waitForDisplayed({timeout:5000})
      await(await(await modal()).$('button=Prepare copies')).click()
      const drag=await(await modal()).$('button=Drag 2 prepared items');await drag.waitForDisplayed({timeout:600_000});await drag.waitForEnabled({timeout:600_000})
      assert.equal((await(await modal()).$$('[role="alert"]')).length,0);assert.equal(await drag.getAttribute('draggable'),'true')
      await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='REAL_CLOUD_DOWNLOAD_PREPARATION'
    })
    const buttonPoint=async()=>{
      const window=await ownedWindow(ui),point=await ui.browser.execute(()=>{const n=[...document.querySelectorAll('[role="dialog"] button')].find(n=>n.textContent.trim()==='Drag 2 prepared items'),r=n.getBoundingClientRect();return [Math.round(r.left+r.width/2),Math.round(r.top+r.height/2)]})
      return point.map((n,i)=>n+window.origin[i])
    }
    await recordPart(result,'cancel-drag',async part=>{
      result.phase='ui';part.ui='STARTED';await observeDrag(ui);const point=await buttonPoint()
      try {part.gesture=await nativeInput(ui,'drag',{points:dragPoints(point,[point[0]+150,point[1]+100]),mode:'copy',modifier_after_press:true,cancel:true})}
      finally {part.events=await endDragObservation(ui)}
      assert.ok(part.events.events.some(e=>e.type==='dragstart'&&e.trusted));await verifyByteTree(fixture,receiver,new Map());part.ui='ACKNOWLEDGED';part.verification='NO_RECEIVER_WRITE_ON_CANCEL'
    })
    await recordPart(result,'receiver-copy',async part=>{
      result.phase='ui';part.ui='STARTED'
      await ui.withNautilus(receiver,async owner=>{
        let target
        await ui.browser.waitUntil(async()=>{try {target=await nativeInput(ui,'place',{pid:owner.pid,executable:owner.executable,data:ui.desktop.env.XDG_DATA_HOME,bounds:[1000,0,940,950]});return true}catch{return false}},{timeout:15_000,interval:100})
        await observeDrag(ui)
        try {part.gesture=await nativeInput(ui,'drag',{points:dragPoints(await buttonPoint(),[target.origin[0]+650,target.origin[1]+700],25),mode:'copy',modifier_after_press:true});await ui.browser.waitUntil(()=>fixture.exists(child(receiver,names[0])),{timeout:30_000,interval:100,timeoutMsg:'No independently observed Nautilus cloud-export copy'})}
        finally {part.events=await endDragObservation(ui)}
        assert.ok(part.events.events.some(e=>e.type==='dragstart'&&e.trusted));part.receiver=await verifyByteTree(fixture,receiver,expected)
      });part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_RECEIVER_BYTES';part.policy='X11 GTK/Nautilus copy only'
    })
    await recordPart(result,'preserved-cloud-and-staging',async part=>{
      result.phase='verification';part.cloud=await verifyByteTree(fixture,source,expected);part.receiver=await verifyByteTree(fixture,receiver,expected)
      const base=path.join(local.run,'profile/data/browsey/cloud-workspaces');await noLinks(base,fs);const dirs=await fs.readdir(base);assert.equal(dirs.length,1);assert.match(dirs[0],/^export-[0-9a-f-]+$/)
      const inputs=path.join(base,dirs[0],'inputs');await noLinks(inputs,fs);assert.deepEqual((await fs.readdir(inputs)).sort(),names.toSorted())
      part.staging=[];for(const [name,bytes]of expected){const actual=await regularFile(path.join(inputs,name));assert.deepEqual(actual.bytes,bytes);part.staging.push({name,bytes:actual.bytes.length})}
      await(await(await modal()).$('button=Close')).click();await ui.idle();await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_ORIGINAL_STAGING_RECEIVER_BYTES';part.cloudOriginalsDeleted=false
    })
  },{id:'desktop-cloud-export',providers:['local','cloud']})
}

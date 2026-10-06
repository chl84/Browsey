/* global window, document, performance, requestAnimationFrame, cancelAnimationFrame */
import assert from 'node:assert/strict'
import path from 'node:path'
import {execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {child} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyTree} from './editing.mjs'
import {releasedResources} from './resources.mjs'
const exec=promisify(execFile)
const withinParts=['within-copy','within-move','within-default','multiple-copy','folder-move','grid-mixed-copy','self-drop','cancel']
export const dragManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local']);return [{id:'desktop-drag',name:'Real owned desktop drags',providers:['local'],partIds:[...withinParts,'between-windows','nautilus-copy','active-teardown']} ]}
export async function nativeInput(ui,command,payload) {
  return JSON.parse((await exec('/usr/bin/python3',[path.join(ui.desktop.repo,'tests/support/native_fixture_x11.py'),command,JSON.stringify(payload)],{env:ui.desktop.env,cwd:ui.desktop.local.files,timeout:15_000,maxBuffer:65536})).stdout)
}
export async function ownedWindow(ui,bounds) {
  const status=await ui.handshake(ui.desktop.plan.runId)
  return nativeInput(ui,bounds?'place':'window',{pid:status.pid,executable:ui.desktop.candidate,data:ui.desktop.env.XDG_DATA_HOME,...(bounds?{bounds}:{})})
}
export async function observeDrag(ui) {
  await ui.browser.execute(async()=>{
    if(window.__nativeDragObservation) throw Error('Observation already active')
    const state={events:[],frames:[],handlers:[],nativeListeners:[],lastPoint:null};window.__nativeDragObservation=state
    const nodes=new WeakMap();let sequence=0
    for(const event of ['tauri://drag-enter','tauri://drag-over','tauri://drag-drop','tauri://drag-leave']) {
      const handler=window.__TAURI_INTERNALS__.transformCallback(e=>{if(e.payload?.position)state.lastPoint={x:e.payload.position.x/(window.devicePixelRatio||1),y:e.payload.position.y/(window.devicePixelRatio||1)};if(state.events.length<1024)state.events.push({type:event,time:performance.now(),payload:e.payload})})
      const eventId=await window.__TAURI_INTERNALS__.invoke('plugin:event|listen',{event,target:{kind:'Webview',label:'main'},handler})
      state.nativeListeners.push({event,eventId})
    }
    for(const type of ['dragstart','dragover','drop','dragend','dragleave']) {
      const handler=e=>{if(type==='dragover'||type==='dragstart')state.lastPoint={x:e.clientX,y:e.clientY};if(state.events.length<1024)state.events.push({type,trusted:e.isTrusted,time:performance.now(),x:e.clientX,y:e.clientY,ctrl:e.ctrlKey,shift:e.shiftKey,effect:e.dataTransfer?.dropEffect,allowed:e.dataTransfer?.effectAllowed,selection:type==='dragstart'?[...document.querySelectorAll('[data-path].selected')].map(n=>n.dataset.path):undefined})}
      document.addEventListener(type,handler,true);state.handlers.push([type,handler])
    }
    const frame=time=>{const ghost=document.querySelector('.ghost');if(ghost&&!nodes.has(ghost))nodes.set(ghost,++sequence);if(state.frames.length<1024)state.frames.push({time,ghost:!!ghost,node:ghost?nodes.get(ghost):null,point:state.lastPoint,text:ghost?.textContent.trim(),left:ghost?.getBoundingClientRect().left,top:ghost?.getBoundingClientRect().top});state.raf=requestAnimationFrame(frame)}
    state.raf=requestAnimationFrame(frame)
  })
}
export async function endDragObservation(ui) {
  return ui.browser.execute(async()=>{const state=window.__nativeDragObservation;if(!state)throw Error('Missing observation');cancelAnimationFrame(state.raf);for(const [type,handler]of state.handlers)document.removeEventListener(type,handler,true);for(const {event,eventId}of state.nativeListeners){window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event,eventId);await window.__TAURI_INTERNALS__.invoke('plugin:event|unlisten',{event,eventId})}delete window.__nativeDragObservation;return {events:state.events,frames:state.frames}})
}
export async function elementPoint(ui,raw) {
  const rect=await ui.browser.execute(raw=>{const node=[...document.querySelectorAll('[data-path]')].find(n=>n.dataset.path===raw&&n.getClientRects().length);if(!node)throw Error('Missing owned drag entry');const r=node.getBoundingClientRect();return [Math.round(r.left+r.width/3),Math.round(r.top+r.height/2)]},raw)
  const window=await ownedWindow(ui);return rect.map((v,i)=>v+window.origin[i])
}
export function dragPoints(a,b,steps=8) {return Array.from({length:steps+1},(_,i)=>a.map((v,j)=>Math.round(v+(b[j]-v)*i/steps)))}
export async function drag(plan,fixture,ui,record) {
  const root=child(plan.targets[0].files,'native-drag');await fixture.mkdir(root)
  await record('Real owned desktop drags',async result=>{
    for(const id of withinParts)await recordPart(result,id,async part=>{
      result.phase='setup';const base=child(root,id),dest=child(base,'destination'),source=child(base,'æ #?%+ spaced source.txt'),bytes=`owned ${id}\n`
      await fixture.mkdir(base);await fixture.mkdir(dest);await fixture.write(source,bytes)
      const tree=child(base,'folder #æ'),second=child(base,'second spaced.txt'),selected=[source],expected=new Map([['destination',null],[source.split('/').at(-1),bytes]])
      if(['multiple-copy','grid-mixed-copy'].includes(id)) {await fixture.write(second,'second\n');selected.push(second);expected.set('second spaced.txt','second\n')}
      if(['folder-move','grid-mixed-copy'].includes(id)) {await fixture.mkdir(tree);await fixture.mkdir(child(tree,'empty'));await fixture.write(child(tree,'nested.txt'),'nested\n');expected.set('folder #æ',null);expected.set('folder #æ/empty',null);expected.set('folder #æ/nested.txt','nested\n');if(id==='folder-move')selected.splice(0,1,tree);else selected.push(tree)}
      const view=id==='grid-mixed-copy'?'grid':'list',mode=id==='within-default'?'default':id.includes('copy')?'copy':'move'
      await ui.navigate(base);await ui.setView(view);await ownedWindow(ui,[0,0,980,900]);await ui.select(selected[0]);for(const raw of selected.slice(1))await ui.modifiedSelect(raw,'Control')
      await ui.selection(base,selected,{directories:selected.includes(tree)?[tree]:[]});await observeDrag(ui)
      result.phase='ui';part.ui='STARTED';const a=await elementPoint(ui,selected[0]),b=await elementPoint(ui,id==='self-drop'?source:dest)
      // A self-drop still crosses the native drag threshold before returning.
      const points=id==='self-drop'?[a,[a[0]+30,a[1]+5],a]:dragPoints(a,b)
      part.gesture=await nativeInput(ui,'drag',{points,mode,modifier_after_press:true,cancel:id==='cancel'})
      let observed
      try {
        if(!['self-drop','cancel'].includes(id)) await ui.browser.waitUntil(()=>fixture.exists(child(dest,selected[0].split('/').at(-1))),{timeout:60_000,interval:100,timeoutMsg:'Native drop created no destination'})
        await ui.idle()
      } finally {observed=await endDragObservation(ui);part.nativeEvents=observed}
      assert.ok(observed.events.some(e=>e.type==='dragstart'&&e.trusted),'Actual trusted WebKit dragstart required')
      await ui.browser.waitUntil(async()=>!await (await ui.browser.$('.ghost')).isExisting(),{timeout:5000,timeoutMsg:'Drag label remained stuck'})
      part.ui='ACKNOWLEDGED';result.phase='verification'
      if(!['self-drop','cancel'].includes(id))for(const raw of selected){const name=raw.split('/').at(-1);for(const [rel,content]of [...expected])if(rel===name||rel.startsWith(name+'/')){expected.set('destination/'+rel,content);if(mode!=='copy')expected.delete(rel)}}
      await verifyTree(fixture,base,expected);await releasedResources(ui,plan.runId);part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'between-windows',async part=>{
      part.ui='STARTED';part.routes=[]
      await ui.withPeer(async peer=>{
        for(const mode of ['default','copy','move']) {
          const base=child(root,'between-'+mode),from=child(base,'source'),to=child(base,'target'),name='æ #?%+ cross window.txt',bytes=`between ${mode}\n`
          await fixture.mkdir(base);await fixture.mkdir(from);await fixture.mkdir(to);await fixture.write(child(from,name),bytes)
          await ui.navigate(from);await ui.setView('list');await ownedWindow(ui,[0,0,940,950])
          await peer.navigate(to);await peer.setView('list');const target=await ownedWindow(peer,[1000,0,940,950])
          await ui.select(child(from,name));await observeDrag(ui);await observeDrag(peer)
          const a=await elementPoint(ui,child(from,name)),b=[target.origin[0]+650,target.origin[1]+650]
          const route={mode};part.routes.push(route)
          try {route.gesture=await nativeInput(ui,'drag',{points:dragPoints(a,b,25),mode,modifier_after_press:true});await peer.idle({resultPath:child(to,name)})}
          finally {route.sourceEvents=await endDragObservation(ui);route.targetEvents=await endDragObservation(peer)}
          assert.ok(route.targetEvents.events.some(e=>e.type==='tauri://drag-drop'),'Actual incoming native drop required')
          await verifyTree(fixture,from,new Map([[name,bytes]]));await verifyTree(fixture,to,new Map([[name,bytes]]));await releasedResources(peer,plan.runId)
          route.negotiated='copy: incoming Browsey policy';route.verification='INDEPENDENT_MATCH'
        }
      })
      part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'nautilus-copy',async part=>{
      result.phase='setup';const base=child(root,'nautilus'),to=child(base,'receiver');await fixture.mkdir(base);await fixture.mkdir(to)
      const expected=new Map();part.routes=[]
      await ui.withNautilus(to,async owner=>{
        let target
        await ui.browser.waitUntil(async()=>{try {target=await nativeInput(ui,'place',{pid:owner.pid,executable:owner.executable,data:ui.desktop.env.XDG_DATA_HOME,bounds:[1000,0,940,950]});return true}catch{return false}},{timeout:15_000,interval:100,timeoutMsg:'Owned Nautilus fixture window did not appear'})
        for(const mode of ['default','copy']) {
          const from=child(base,mode),first=child(from,mode+' æ #?%+ file.txt'),second=child(from,mode+' second.txt'),tree=child(from,mode+' folder')
          await fixture.mkdir(from);await fixture.write(first,'first\n');await fixture.write(second,'second\n');await fixture.mkdir(tree);await fixture.mkdir(child(tree,'empty'));await fixture.write(child(tree,'nested.txt'),'nested\n')
          const shape=new Map([[first.split('/').at(-1),'first\n'],[second.split('/').at(-1),'second\n'],[tree.split('/').at(-1),null],[tree.split('/').at(-1)+'/empty',null],[tree.split('/').at(-1)+'/nested.txt','nested\n']])
          await ui.navigate(from);await ui.setView('list');await ownedWindow(ui,[0,0,940,950]);await ui.select(first);await ui.modifiedSelect(second,'Control');await ui.modifiedSelect(tree,'Control');await ui.selection(from,[first,second,tree],{directories:[tree]});await observeDrag(ui)
          const route={mode};part.routes.push(route);part.ui='STARTED'
          try {route.gesture=await nativeInput(ui,'drag',{points:dragPoints(await elementPoint(ui,first),[target.origin[0]+650,target.origin[1]+700],25),mode,modifier_after_press:true});await ui.browser.waitUntil(()=>fixture.exists(child(to,first.split('/').at(-1))),{timeout:30_000,interval:100,timeoutMsg:'Nautilus created no independently observed copy'})}
          finally {route.sourceEvents=await endDragObservation(ui)}
          for(const entry of shape)expected.set(...entry)
          await verifyTree(fixture,from,shape);await verifyTree(fixture,to,expected);route.verification='INDEPENDENT_MATCH';route.negotiated='copy (Nautilus X11 policy)'
        }
      });part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'active-teardown',async part=>{
      const base=child(root,'active-teardown'),name='preserved.txt';await fixture.mkdir(base);await fixture.write(child(base,name),'preserved\n');await ui.navigate(base);await ui.setView('list');await ownedWindow(ui,[0,0,940,950]);await ui.select(child(base,name));await observeDrag(ui)
      const a=await elementPoint(ui,child(base,name));part.gesture=await nativeInput(ui,'hold',{points:dragPoints(a,[a[0]+130,a[1]+200]),mode:'default'})
      part.nativeEvents=await endDragObservation(ui);assert.ok(part.nativeEvents.events.some(e=>e.type==='dragstart'&&e.trusted),'Teardown requires a real active source drag')
      ui.afterRestartStop=async()=>{part.release=await nativeInput(ui,'release',{})}
      await ui.restart();await ui.navigate(base);await verifyTree(fixture,base,new Map([[name,'preserved\n']]));part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
  },{id:'desktop-drag',providers:['local']})
}

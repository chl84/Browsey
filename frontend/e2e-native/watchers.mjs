/* global window */
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {child,ownedPath,noLinks} from './scope.mjs'
import {recordPart} from './report.mjs'
import {regularFile} from './fixtures.mjs'
import {verifyTree} from './editing.mjs'
import {progressLocations,progressProbes,beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {releasedResources} from './resources.mjs'
const parts=['external-create','external-edit','external-rename','external-delete','refresh-during-work','queued-change-shutdown']
export const watchersManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local']);return [{id:'desktop-watchers',name:'Fixture-only real watcher changes and shutdown',providers:['local'],partIds:parts}]}
export const watchersProbes=plan=>[{...progressProbes(plan)[0],holdMs:5000,slowMs:0}]
async function observe(ui) {
  await ui.browser.execute(async()=>{if(window.__ownedWatchObservation)throw Error('Watcher observer already exists');const state={events:[]};window.__ownedWatchObservation=state;state.handler=window.__TAURI_INTERNALS__.transformCallback(e=>{if(state.events.length<64)state.events.push({path:e.payload,time:Date.now()})});state.id=await window.__TAURI_INTERNALS__.invoke('plugin:event|listen',{event:'dir-changed',target:{kind:'Webview',label:'main'},handler:state.handler})})
}
async function finish(ui) {return ui.browser.execute(async()=>{const s=window.__ownedWatchObservation;if(!s)throw Error('Missing watcher observer');window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener('dir-changed',s.id);await window.__TAURI_INTERNALS__.invoke('plugin:event|unlisten',{event:'dir-changed',eventId:s.id});delete window.__ownedWatchObservation;return s.events})}
async function eventCount(ui) {return ui.browser.execute(()=>window.__ownedWatchObservation.events.length)}
async function change(ui,action) {const before=await eventCount(ui);await action();await ui.browser.waitUntil(async()=>await eventCount(ui)>before,{timeout:10_000,interval:100,timeoutMsg:'No actual filesystem watcher event arrived'})}
async function replace(fixture,raw,before,after) {ownedPath(fixture.roots,raw);assert.equal((await regularFile(raw)).text,before);await noLinks(raw,fs);const handle=await fs.open(raw,fs.constants.O_WRONLY|fs.constants.O_NOFOLLOW);try{await handle.truncate(0);await handle.writeFile(after)}finally{await handle.close()}}
export async function watchers(plan,fixture,ui,record) {
  await record('Fixture-only real watcher changes and shutdown',async result=>{
    result.phase='setup';const base=child(plan.targets[0].files,'watchers'),raw=child(base,'external.txt'),renamed=child(base,'renamed.txt'),sentinel=child(base,'sentinel.txt');await fixture.mkdir(base);await fixture.write(sentinel,'preserve\n');await ui.navigate(base);await ui.setView('list');await observe(ui)
    const state=await ui.handshake(plan.runId);assert.equal(state.desktopMode,'watchers');assert.equal(state.watcher,true);assert.equal(state.watcherActive,true,'Actual recommended watcher must be installed')
    for(const id of parts.slice(0,4))await recordPart(result,id,async part=>{
      result.phase='ui';part.ui='STARTED';const before=await eventCount(ui)
      if(id==='external-create'){await change(ui,()=>fixture.write(raw,'original\n'));await ui.listing(base,'list',[raw,sentinel])}
      if(id==='external-edit'){await change(ui,()=>replace(fixture,raw,'original\n','longer external edit\n'));await ui.browser.waitUntil(async()=>{const row=await ui.browser.$(`[data-path=${JSON.stringify(raw)}]`);return(await row.getText()).includes('21 B')},{timeout:10_000,interval:100,timeoutMsg:'External edit metadata did not update without refresh'})}
      if(id==='external-rename'){await change(ui,async()=>{ownedPath(fixture.roots,renamed);await noLinks(raw,fs);await noLinks(renamed,fs);await fs.rename(raw,renamed)});await ui.listing(base,'list',[renamed,sentinel])}
      if(id==='external-delete'){await change(ui,async()=>{assert.equal((await regularFile(renamed)).text,'longer external edit\n');await fs.unlink(renamed)});await ui.listing(base,'list',[sentinel])}
      part.events=(await eventCount(ui))-before;part.ui='ACKNOWLEDGED';result.phase='verification';await verifyTree(fixture,base,new Map([['sentinel.txt','preserve\n'],...(id==='external-create'?[['external.txt','original\n']]:id==='external-edit'?[['external.txt','longer external edit\n']]:id==='external-rename'?[['renamed.txt','longer external edit\n']]:[])]));part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'refresh-during-work',async part=>{
      result.phase='setup';const p=progressLocations(plan,{from:'local',to:'local'},'file');await fixture.mkdir(p.from);await fixture.mkdir(p.to);await fixture.write(p.source,'watcher transfer\n');await ui.populateClipboard(p.from,[p.source],false);result.phase='ui';await beginPaste(ui,p.to);await(await ui.browser.$('.pill.progress')).waitForDisplayed({timeout:5000});const extra=child(p.to,'external-during-copy.txt');await change(ui,()=>fixture.write(extra,'external\n'));await ui.refresh();await ui.idle({resultPath:p.target});await waitActivityGone(ui);part.progress=await endTransferObservation(ui);assert.ok((await ui.handshake(plan.runId)).probes.find(x=>x.id===p.id)?.consumed);result.phase='verification';await verifyTree(fixture,p.from,new Map([[p.name,'watcher transfer\n']]));await verifyTree(fixture,p.to,new Map([[p.name,'watcher transfer\n'],['external-during-copy.txt','external\n']]));part.ui='ACKNOWLEDGED'
    })
    await recordPart(result,'queued-change-shutdown',async part=>{
      result.phase='ui';await ui.navigate(base);part.events=await finish(ui);assert.ok(part.events.length&&part.events.length<=64);for(const event of part.events)ownedPath(fixture.roots,event.path);await fixture.write(child(base,'queued.txt'),'queued\n');const old=(await ui.handshake(plan.runId)).pid;await ui.restart();assert.notEqual((await ui.handshake(plan.runId)).pid,old);assert.equal((await ui.handshake(plan.runId)).watcher,true);await ui.navigate(base);assert.equal((await ui.handshake(plan.runId)).watcherActive,true);await ui.listing(base,'list',[sentinel,child(base,'queued.txt')]);result.phase='verification';await verifyTree(fixture,base,new Map([['sentinel.txt','preserve\n'],['queued.txt','queued\n']]));await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
  },{id:'desktop-watchers',providers:['local']})
}

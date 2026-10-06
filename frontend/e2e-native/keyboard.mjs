/* global document */
import assert from 'node:assert/strict'
import {Key} from 'webdriverio'
import {child} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyTree} from './editing.mjs'
import {progressLocations,progressProbes,beginPaste,endTransferObservation,waitActivityGone} from './progress.mjs'
import {releasedResources} from './resources.mjs'
const parts=['list-navigation','grid-navigation','creation-tab-cycle','focused-escape-restore','context-menu-keys','properties-names-restore','tooltip-escape','slow-operation-focus']
export const keyboardManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local']);return [{id:'desktop-keyboard',name:'Native keyboard, focus and accessible names',providers:['local'],partIds:parts}]}
export const keyboardProbes=plan=>[{...progressProbes(plan)[0],holdMs:5000,slowMs:0}]
async function focus(ui) {return ui.browser.execute(()=>{const n=document.activeElement;return {tag:n?.tagName,id:n?.id,role:n?.getAttribute('role'),label:n?.getAttribute('aria-label'),text:n?.textContent?.trim(),path:n?.closest('[data-path]')?.dataset.path,dialog:!!n?.closest('[role="dialog"]'),menu:!!n?.closest('[role="menu"]')}})}
async function dialogTabs(ui) {
  const count=await ui.browser.execute(()=>[...document.querySelector('[role="dialog"]').querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])')].filter(n=>n.getClientRects().length).length)
  assert.ok(count>=2&&count<=20);const samples=[]
  for(let i=0;i<=count;i++){await ui.browser.keys([Key.Tab]);const s=await focus(ui);assert.ok(s.dialog,'Tab must remain inside the modal');samples.push(s)}
  assert.deepEqual(samples[0],samples.at(-1),'One complete Tab cycle must return to the same control')
  await ui.browser.keys([Key.Shift,Key.Tab,Key.Shift]);assert.ok((await focus(ui)).dialog,'Shift+Tab must remain inside the modal');return samples
}
export async function keyboard(plan,fixture,ui,record) {
  await record('Native keyboard, focus and accessible names',async result=>{
    assert.equal(typeof ui.accessibilitySnapshot,'function','Dynamic actual AT-SPI evidence required')
    result.phase='setup';const base=child(plan.targets[0].files,'keyboard'),names=['a.txt','b.txt','c.txt'],paths=names.map(n=>child(base,n)),expected=new Map(names.map(n=>[n,n+'\n']))
    await fixture.mkdir(base);for(const [name,bytes]of expected)await fixture.write(child(base,name),bytes)
    for(const view of ['list','grid'])await recordPart(result,view+'-navigation',async part=>{
      result.phase='ui';await ui.navigate(base);await ui.setView('list');await ui.sort('Name','asc');await ui.setView(view);await ui.select(paths[0]);await ui.selectionKey(view==='list'?'ArrowDown':'ArrowRight');await ui.selection(base,[paths[1]]);await ui.selectionKey(view==='list'?'ArrowDown':'ArrowRight','Shift');await ui.selection(base,paths.slice(1));await ui.selectionKey('Escape');await ui.selection(base,[]);part.ui='ACKNOWLEDGED';part.focus=await focus(ui)
    })
    await ui.setView('list')
    await recordPart(result,'creation-tab-cycle',async part=>{result.phase='ui';await ui.beginCreation(base,false);await ui.creationValue(false,'cancelled.txt');part.tabs=await dialogTabs(ui);part.accessibility=await ui.accessibilitySnapshot();assert.ok(part.accessibility.some(n=>n[1]==='Cancel'));assert.ok(part.accessibility.some(n=>n[1]==='Create'));await ui.cancelCreation(false);await ui.creationFocus(base);part.ui='ACKNOWLEDGED'})
    await recordPart(result,'focused-escape-restore',async part=>{result.phase='ui';await ui.beginCreation(base,false);await ui.creationValue(false,'escaped.txt');await ui.cancelCreation(false,true);await ui.creationFocus(base);part.focus=await focus(ui);part.ui='ACKNOWLEDGED'})
    await recordPart(result,'context-menu-keys',async part=>{result.phase='ui';await ui.select(paths[0]);await(await ui.browser.$(`[data-path=${JSON.stringify(paths[0])}]`)).click({button:'right'});await ui.browser.waitUntil(async()=>(await focus(ui)).menu,{timeout:5000});const before=await focus(ui);await ui.browser.keys([Key.ArrowDown]);assert.notDeepEqual(await focus(ui),before);await ui.browser.keys([Key.Home]);assert.deepEqual(await focus(ui),before);await ui.browser.keys([Key.Escape]);await ui.idle();assert.equal(await(await ui.browser.$('[role="menu"]')).isExisting(),false);await ui.editingFocus(base,[paths[0]]);part.ui='ACKNOWLEDGED'})
    await recordPart(result,'properties-names-restore',async part=>{result.phase='ui';await ui.propertiesOpen(base,[paths[0]],true);part.tabs=await dialogTabs(ui);part.accessibility=await ui.accessibilitySnapshot();for(const name of ['Properties','Basic','Extra','Ownership','Permissions','Copy parent folder path'])assert.ok(part.accessibility.some(n=>n[1]===name),'AT-SPI name missing: '+name);await ui.closeProperties(base);await ui.editingFocus(base,[paths[0]]);part.ui='ACKNOWLEDGED'})
    await recordPart(result,'tooltip-escape',async part=>{result.phase='ui';await ui.propertiesOpen(base,[paths[0]]);await(await ui.browser.$('[aria-label="Copy parent folder path"]')).moveTo();const tip=await ui.browser.$('.browsey-tooltip');await tip.waitForDisplayed({timeout:5000});part.text=await tip.getText();assert.ok(part.text.length);await ui.browser.keys([Key.Escape]);await tip.waitForExist({reverse:true,timeout:5000});await ui.idle();await ui.editingFocus(base,[paths[0]]);part.ui='ACKNOWLEDGED'})
    await recordPart(result,'slow-operation-focus',async part=>{
      result.phase='setup';const p=progressLocations(plan,{from:'local',to:'local'},'file');await fixture.mkdir(p.from);await fixture.mkdir(p.to);await fixture.write(p.source,'slow\n');await ui.populateClipboard(p.from,[p.source],false);result.phase='ui';await beginPaste(ui,p.to);await(await ui.browser.$('.pill.progress')).waitForDisplayed({timeout:5000});await(await ui.browser.$('[aria-label="Main menu"]')).click();const menu=await ui.browser.$('[role="menu"][aria-label="Main actions"]');await menu.waitForDisplayed({timeout:5000});await ui.browser.waitUntil(async()=>(await focus(ui)).menu,{timeout:5000});part.menuFocus=await focus(ui);await ui.browser.keys([Key.Escape]);await menu.waitForExist({reverse:true,timeout:5000});await ui.browser.waitUntil(async()=>(await focus(ui)).label==='Main menu',{timeout:5000,timeoutMsg:'Escape must return focus to the main menu opener'});await(await ui.browser.$('[aria-label="Main menu"]')).click();await menu.waitForDisplayed({timeout:5000});await(await menu.$('[role="switch"]')).click();await menu.waitForExist({reverse:true,timeout:5000});await ui.idle({resultPath:p.target});await waitActivityGone(ui);part.progress=await endTransferObservation(ui);assert.ok(part.progress.some(s=>s.visible&&s.active));const probe=(await ui.handshake(plan.runId)).probes.find(x=>x.id===p.id);assert.ok(probe?.consumed);part.checkpoint=probe;result.phase='verification';await verifyTree(fixture,p.from,new Map([[p.name,'slow\n']]));await verifyTree(fixture,p.to,new Map([[p.name,'slow\n']]));await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
    result.phase='verification';await verifyTree(fixture,base,expected)
  },{id:'desktop-keyboard',providers:['local']})
}

/* global document, window */
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {pathToFileURL,fileURLToPath} from 'node:url'
import {child} from './scope.mjs'
import {regularFile} from './fixtures.mjs'
import {verifyTree} from './editing.mjs'
import {recordPart} from './report.mjs'
import {releasedResources} from './resources.mjs'
const exec=promisify(execFile),parts=['clipboard-cut-clear','clipboard-copy','trash-mixed','restore-selected','purge-cancel','purge-selected','empty-cancel','empty-private-trash']
export const servicesManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local']);return [{id:'desktop-services',name:'Isolated clipboard and private trash',providers:['local'],partIds:parts}]}
async function clipboard(ui) {
  const result=await exec(path.join(ui.desktop.repo,'target/native-tools/bin/xclip'),['-selection','clipboard','-t','text/uri-list','-o'],{env:ui.desktop.env,timeout:3000,maxBuffer:65536})
  const lines=result.stdout.split(/\r?\n/).filter(Boolean),paths=lines.filter(s=>!s.startsWith('#')).map(s=>fileURLToPath(s))
  return {mode:lines.includes('#cut')?'cut':'copy',paths,payload:result.stdout}
}
async function trashCatalog(ui) {
  const root=path.join(ui.desktop.env.XDG_DATA_HOME,'Trash'),info=path.join(root,'info')
  let names
  try {names=await fs.readdir(info)}catch(error){if(error.code==='ENOENT')return [];throw error}
  assert.ok(names.length<=16);const entries=[]
  for(const name of names) {
    assert.ok(name.endsWith('.trashinfo'))
    const text=(await regularFile(path.join(info,name))).text,original=text.split('\n').find(s=>s.startsWith('Path='))?.slice(5)
    const raw=fileURLToPath('file://'+original);assert.ok(raw.startsWith(ui.desktop.local.files+'/'))
    entries.push({id:path.join(info,name),original:raw,path:path.join(root,'files',name.slice(0,-10))})
  }
  return entries
}
async function trashSelect(ui,original) {
  const entry=(await trashCatalog(ui)).find(e=>e.original===original);assert.ok(entry,'Known original private trash entry required')
  await(await ui.browser.$(`[data-path=${JSON.stringify(entry.path)}]`)).click();return entry
}
async function trashAction(ui,id,entry) {
  assert.ok(['restore','delete-permanent'].includes(id));assert.ok((await trashCatalog(ui)).some(e=>e.id===entry.id&&e.path===entry.path))
  await(await ui.browser.$(`[data-path=${JSON.stringify(entry.path)}]`)).click({button:'right'})
  const action=await ui.browser.$(`[role="menuitem"][data-action-id="${id}"]`);await action.waitForDisplayed({timeout:5000});await action.click()
}
async function openWastebasket(ui) {
  await(await ui.browser.$('.nav[data-drop-path="trash://"]')).click();await ui.idle()
  const paths=await ui.browser.execute(()=>[...document.querySelectorAll('.rows [data-path], .grid [data-path]')].filter(n=>n.getClientRects().length).map(n=>n.dataset.path))
  const known=(await trashCatalog(ui)).map(e=>e.path);assert.deepEqual(paths.sort(),known.sort(),'Only independently bound private trash entries may appear')
}
async function emptyPrompt(ui) {
  await(await ui.browser.$('.nav[data-drop-path="trash://"]')).click({button:'right'})
  await(await ui.browser.$('[data-action-id="empty-wastebasket"]')).click()
  const dialog=await ui.browser.$('[role="dialog"]');await dialog.waitForDisplayed({timeout:5000});return dialog
}
export async function services(plan,fixture,ui,record) {
  await record('Isolated clipboard and private trash',async result=>{
    assert.equal((await ui.handshake(plan.runId)).desktopMode,'desktop-services','Real approved backend mode required')
    await ui.withPeer(async peer=>{
      for(const mode of ['cut','copy'])await recordPart(result,mode==='cut'?'clipboard-cut-clear':'clipboard-copy',async part=>{
        result.phase='setup';const base=child(plan.targets[0].files,'clipboard-'+mode),from=child(base,'source'),to=child(base,'target'),name='æ #?%+ clipboard file.txt'
        await fixture.mkdir(base);await fixture.mkdir(from);await fixture.mkdir(to);await fixture.write(child(from,name),'clipboard\n');await fixture.write(child(from,'second.txt'),'second\n');const sourcePaths=[child(from,name),child(from,'second.txt')],expectedClipboard=new Map([[name,'clipboard\n'],['second.txt','second\n']])
        await ui.populateClipboard(from,sourcePaths,mode==='cut');let content
        await ui.browser.waitUntil(async()=>{try {content=await clipboard(ui);return content.mode===mode&&JSON.stringify([...content.paths].sort())===JSON.stringify([...sourcePaths].sort())}catch{return false}},{timeout:15_000,interval:100,timeoutMsg:'Actual private X clipboard payload did not match copied paths'})
        part.clipboard={mode:content.mode,paths:content.paths,uri:pathToFileURL(child(from,name)).href};part.ui='STARTED';result.phase='ui'
        part.imported=await peer.browser.execute(()=>window.__TAURI_INTERNALS__.invoke('system_clipboard_paths'))
        assert.equal(part.imported.mode,mode);assert.deepEqual([...part.imported.paths].sort(),[...sourcePaths].sort(),'Real backend clipboard mode/paths must match the independent X selection')
        await peer.paste(to,child(to,name));await peer.idle()
        result.phase='verification'
        await verifyTree(fixture,to,expectedClipboard);await verifyTree(fixture,from,mode==='cut'?new Map():expectedClipboard)
        if(mode==='cut'){await ui.browser.waitUntil(async()=>{const cleared=await exec(path.join(ui.desktop.repo,'target/native-tools/bin/xclip'),['-selection','clipboard','-o'],{env:ui.desktop.env,timeout:3000,maxBuffer:65536});return cleared.stdout.length===0},{timeout:5000,interval:100,timeoutMsg:'Cut completion did not clear the real clipboard'});part.cleared=true}
        await releasedResources(peer,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
      })
    })
    const base=child(plan.targets[0].files,'private-trash'),file=child(base,'æ #?%+ restore.txt'),tree=child(base,'purge folder'),sentinel=child(base,'sentinel.txt'),expected=new Map([['æ #?%+ restore.txt','restore\n'],['purge folder',null],['purge folder/nested.txt','nested\n'],['sentinel.txt','preserve\n']])
    await fixture.mkdir(base);await fixture.write(file,'restore\n');await fixture.mkdir(tree);await fixture.write(child(tree,'nested.txt'),'nested\n');await fixture.write(sentinel,'preserve\n')
    await recordPart(result,'trash-mixed',async part=>{
      await ui.navigate(base);await ui.setView('list');await ui.select(file);await ui.modifiedSelect(tree,'Control');await ui.selection(base,[file,tree],{directories:[tree]});result.phase='ui';await ui.menuAction('move-trash',file);await ui.idle({absentPath:file})
      result.phase='verification';const entries=await trashCatalog(ui);assert.deepEqual(entries.map(e=>e.original).sort(),[file,tree].sort());assert.equal((await regularFile(entries.find(e=>e.original===file).path)).text,'restore\n');assert.equal((await regularFile(child(entries.find(e=>e.original===tree).path,'nested.txt'))).text,'nested\n')
      expected.delete('æ #?%+ restore.txt');expected.delete('purge folder');expected.delete('purge folder/nested.txt');await verifyTree(fixture,base,expected);part.catalog=entries;part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'restore-selected',async part=>{result.phase='ui';await openWastebasket(ui);await trashAction(ui,'restore',await trashSelect(ui,file));await ui.idle();expected.set('æ #?%+ restore.txt','restore\n');await verifyTree(fixture,base,expected);assert.deepEqual((await trashCatalog(ui)).map(e=>e.original),[tree]);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'})
    await recordPart(result,'purge-cancel',async part=>{result.phase='ui';
      await openWastebasket(ui);await trashAction(ui,'delete-permanent',await trashSelect(ui,tree));const dialog=await ui.browser.$('[role="dialog"]');await dialog.waitForDisplayed({timeout:5000});part.warning=await dialog.getText();result.phase='verification';assert.match(part.warning,/permanently removed from the Wastebasket/i);assert.match(part.warning,/This cannot be undone/i);assert.doesNotMatch(part.warning,/can undo supported local deletions/i);await(await ui.browser.$('[data-cancel-delete="1"]')).click();await ui.idle();assert.equal((await trashCatalog(ui)).length,1);await verifyTree(fixture,base,expected);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
    await recordPart(result,'purge-selected',async part=>{await trashAction(ui,'delete-permanent',await trashSelect(ui,tree));await(await ui.browser.$('[data-confirm-delete="1"]')).click();await ui.idle();assert.deepEqual(await trashCatalog(ui),[]);await verifyTree(fixture,base,expected);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'})
    const emptyA=child(base,'empty-a.txt'),emptyB=child(base,'empty-b.txt');await fixture.write(emptyA,'a\n');await fixture.write(emptyB,'b\n');await ui.navigate(base);await ui.select(emptyA);await ui.modifiedSelect(emptyB,'Control');await ui.menuAction('move-trash',emptyA);await ui.idle({absentPath:emptyA});await openWastebasket(ui)
    await recordPart(result,'empty-cancel',async part=>{const dialog=await emptyPrompt(ui);await(await dialog.$('.//button[normalize-space(.)="Cancel"]')).click();await ui.idle();assert.equal((await trashCatalog(ui)).length,2);await verifyTree(fixture,base,expected);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'})
    await recordPart(result,'empty-private-trash',async part=>{const dialog=await emptyPrompt(ui);await(await dialog.$('.//button[normalize-space(.)="Empty Wastebasket"]')).click();await ui.idle();assert.deepEqual(await trashCatalog(ui),[]);assert.deepEqual(await fs.readdir(path.join(ui.desktop.env.XDG_DATA_HOME,'Trash/files')),[]);await verifyTree(fixture,base,expected);await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'})
  },{id:'desktop-services',providers:['local']})
}

/* global window */
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {constants} from 'node:fs'
import {child,noLinks,inside} from './scope.mjs'
import {privateJson,privateStat} from './privacy.mjs'
import {regularFile} from './fixtures.mjs'

export async function workingCopies(plan,source) {
  const local=plan.targets.find(t=>t.kind==='local'),root=['profile','data','browsey','cloud-workspaces'].reduce(child,local.run)
  await noLinks(root,fs);privateStat(await fs.lstat(root),true)
  const names=await fs.readdir(root);assert.ok(names.length<=16,'Owned working-copy listing is bounded')
  const result=[]
  for(const id of names) {
    if(id.startsWith('source-check-')) continue
    assert.match(id,/^[a-f\d-]{1,128}$/i)
    const dir=child(root,id),copy=await privateJson(child(dir,'manifest.json'))
    assert.equal(copy.id,id);assert.equal(copy.sourcePath,source)
    const file=child(child(dir,'files'),source.split('/').at(-1));assert.ok(inside(root,file));assert.equal(copy.localPath,file)
    const actual=await regularFile(file);privateStat(actual.stat);assert.ok(actual.bytes.length<=65536)
    result.push({...copy,bytes:actual.bytes})
  }
  return result
}
export async function editWorkingCopy(plan,source,id,before,after) {
  assert.ok(Buffer.byteLength(after)<=65536)
  const copy=(await workingCopies(plan,source)).find(c=>c.id===id);assert.ok(copy)
  assert.deepEqual(copy.bytes,Buffer.from(before),'Refuse an unexpected working-copy version before editing')
  await noLinks(copy.localPath,fs)
  const handle=await fs.open(copy.localPath,constants.O_RDWR|constants.O_NOFOLLOW)
  try {
    const stat=await handle.stat();privateStat(stat);assert.ok(stat.size<=65536)
    assert.deepEqual(await handle.readFile(),copy.bytes)
    await handle.truncate(0);await handle.write(Buffer.from(after),0,Buffer.byteLength(after),0)
  } finally {await handle.close()}
}
export async function workspaceCallbacks(ui) {
  return ui.browser.execute(()=>{
    const listeners=window.__internal_unstable_listeners_object_id__,callbacks=window.__TAURI_INTERNALS__?.callbacks
    if(!listeners||typeof callbacks?.has!=='function') throw Error('Native listener registries unavailable')
    return Object.getOwnPropertyNames(listeners).filter(name=>/^(cloud-open-|cloud-edit-upload-)/.test(name))
      .flatMap(name=>Object.getOwnPropertyNames(listeners[name]).map(id=>listeners[name][id].handlerId).filter(id=>callbacks.has(id)).map(id=>({event:name,handler:id})))
  })
}
export async function releasedWorkspace(ui,runId) {
  await ui.browser.waitUntil(async()=>!(await workspaceCallbacks(ui)).length&&(await ui.handshake(runId)).cancelTasks===0,
    {timeout:180_000,interval:100,timeoutMsg:'Cloud working-copy resources did not release'})
}
export async function uploadEditedCopy(ui,source,{changed}) {
  await ui.chord('s');const settings=await ui.browser.$('.settings-modal');await settings.waitForDisplayed({timeout:10_000})
  await ui.fill(await settings.$('.settings-filter'),'cloud')
  const button=await settings.$('.//button[normalize-space(.)="Working copies…"]');await button.waitForDisplayed({timeout:10_000});await button.click()
  const dialog=await ui.browser.$('//*[@role="dialog"][header[normalize-space(.)="Cloud working copies"]]')
  await dialog.waitForDisplayed({timeout:10_000})
  await (await dialog.$('.copies[aria-busy="false"]')).waitForExist({timeout:180_000})
  const modified=await dialog.$$('.//section[.//small[contains(.,"Locally modified")]]');assert.equal(modified.length,1)
  assert.equal(await (await modified[0].$('.source')).getText(),source)
  const upload=await modified[0].$('.//button[normalize-space(.)="Upload changes as new file"]');assert.ok(await upload.isEnabled());await upload.click()
  let message
  await ui.browser.waitUntil(async()=>{
    const error=await dialog.$('.error[role="alert"]');if(await error.isExisting()) throw Error(await error.getText())
    const status=await dialog.$('[role="status"]');message=await status.isExisting()?await status.getText():''
    return message.includes('Saved as a new file:')
  },{timeout:600_000,interval:150,timeoutMsg:'Explicit edited-copy upload did not complete'})
  assert.equal(message.startsWith('The cloud original changed.'),changed)
  const path=message.match(/Saved as a new file: (.+)\. The original and working copy were kept\.$/)?.[1]
  assert.ok(path&&path.startsWith(source.slice(0,source.lastIndexOf('/')+1))&&path!==source)
  await (await dialog.$('.//button[normalize-space(.)="Close"]')).click()
  await ui.browser.keys(['Escape']);await ui.browser.keys(['Escape']);await settings.waitForExist({reverse:true,timeout:10_000});await ui.idle()
  return path
}

/* global window, document, MutationObserver, performance */
import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'

const parts=['file','tree','zero']
export const progressRoutes=plan=>[{from:'local',to:'local'},...plan.targets.filter(t=>t.kind!=='local')
  .flatMap(t=>[{from:'local',to:t.kind},{from:t.kind,to:'local'}])]
const idFor=c=>`progress-${c.from}-${c.to}`
export const progressManifest=plan=>progressRoutes(plan).map(c=>({id:idFor(c),name:`Byte progress: ${c.from} to ${c.to}`,
  providers:[...new Set([c.from,c.to])],partIds:parts}))
export function progressLocations(plan,c,part) {
  const id=`${idFor(c)}-${part}`,from=child(plan.targets.find(t=>t.kind===c.from).files,`${id}-source`),
    to=child(plan.targets.find(t=>t.kind===c.to).files,`${id}-target`),name=part==='tree'?'tree':part==='zero'?'zero.bin':'body.txt'
  return {id,from,to,name,source:child(from,name),target:child(to,name)}
}
export const progressProbes=plan=>progressRoutes(plan).flatMap(c=>parts.map(part=>{
  const p=progressLocations(plan,c,part)
  return {id:p.id,source:p.source,target:p.target,holdPhase:'finalize',holdBytes:0,holdMs:1800,slowMs:250}
}))

// This observer records displayed state; operations still use real controls.
export async function startTransferObservation(ui) {
  await ui.browser.execute(()=>{
    if(window.__browseyTransferObservation) throw Error('Transfer observer already exists')
    const samples=[];let lastSignature,cancelRequestedAt=null
    const capture=()=>{
      const pill=document.querySelector('.pill.progress'),main=document.querySelector('main.shell')
      const state={label:pill?.querySelector('span')?.textContent??null,detail:pill?.querySelector('.detail')?.textContent??null,
        percent:pill?.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')??null,
        active:main?.getAttribute('data-operation-active')==='true',visible:!!pill?.getClientRects().length,
        cancel:!!pill?.querySelector('[aria-label="Cancel task"]'),toast:document.querySelector('.toast[role="status"]')?.textContent.trim()??'',cancelRequestedAt}
      const signature=JSON.stringify(state)
      if(samples.length<256&&lastSignature!==signature) {samples.push({...state,time:performance.now()});lastSignature=signature}
    }
    const click=event=>{if(event.isTrusted&&event.target.closest?.('[aria-label="Cancel task"]')){cancelRequestedAt=performance.now();capture()}}
    document.addEventListener('click',click,true)
    const observer=new MutationObserver(capture)
    observer.observe(document.body,{subtree:true,childList:true,characterData:true,attributes:true})
    window.__browseyTransferObservation={samples,observer,capture,click};capture()
  })
}
export async function endTransferObservation(ui) {
  return ui.browser.execute(()=>{
    const state=window.__browseyTransferObservation
    if(!state) throw Error('No owned transfer observer')
    state.capture();state.observer.disconnect();document.removeEventListener('click',state.click,true);delete window.__browseyTransferObservation;return state.samples
  })
}
export async function beginPaste(ui,to,{conflict}={}) {
  await ui.navigate(to);await ui.emptySpace(to);await startTransferObservation(ui);await ui.burst('v',1)
  if(conflict) {
    const dialog=await ui.browser.$('.conflict-modal');await dialog.waitForDisplayed({timeout:180_000})
    await (await dialog.$(`.//button[normalize-space(.)="${conflict}"]`)).click()
  }
}
export function assertProgress(samples,{known,zero}) {
  assert.ok(samples.some(s=>s.visible&&s.active),'Activity must be visible while the transfer is active')
  assert.ok(samples.every(s=>!/(^|\s)1 B(?:\s|$)/.test(s.detail??'')),'Unknown totals must not masquerade as 1 byte')
  if(known&&!zero) assert.ok(samples.some(s=>s.visible&&s.active&&s.detail?.includes('64.0 KB / 64.0 KB')),
    'Real file bytes must be visible before command completion')
  if(!known||zero) assert.ok(samples.some(s=>s.visible&&s.active&&s.percent===null),'Unknown/zero totals stay indeterminate')
  assert.ok(samples.every(s=>!s.active||!s.label?.includes('completed')),'A child cannot complete the operation')
  assert.equal(samples.at(-1).visible,false,'Activity must clear after finalization')
}
export async function waitActivityGone(ui) {
  await ui.browser.waitUntil(async()=>!await ui.browser.execute(()=>document.querySelector('.pill.progress')?.getClientRects().length),
    {timeout:10_000,interval:100,timeoutMsg:'Transfer left stale activity'})
}

export async function progress(plan,fixture,ui,record) {
  for(const c of progressRoutes(plan)) await record(`Byte progress: ${c.from} to ${c.to}`,async result=>{
    await ui.setView('list')
    for(const partId of parts) await recordPart(result,partId,async part=>{
      result.phase='setup'
      const p=progressLocations(plan,c,partId),source=new Map(),target=new Map()
      await fixture.mkdir(p.from);await fixture.mkdir(p.to)
      const content=`${p.id}\n`.padEnd(65536,'x')
      if(partId==='tree') {
        await fixture.mkdir(p.source);await fixture.mkdir(child(p.source,'empty'));await fixture.write(child(p.source,'inside.txt'),content)
        source.set('tree',null);source.set('tree/empty',null);source.set('tree/inside.txt',content)
      } else {await fixture.write(p.source,partId==='zero'?'':content);source.set(p.name,partId==='zero'?'':content)}
      await fixture.write(child(p.to,'sentinel.txt'),'unrelated target\n');target.set('sentinel.txt','unrelated target\n')
      await ui.populateClipboard(p.from,[p.source],false,false,{directories:partId==='tree'?[p.source]:[]})
      result.phase='ui';part.ui='STARTED';await beginPaste(ui,p.to)
      try {await ui.idle({resultPath:p.target},600_000);await waitActivityGone(ui)}
      catch(error) {part.progress=await endTransferObservation(ui);throw error}
      part.progress=await endTransferObservation(ui);part.ui='ACKNOWLEDGED';result.phase='verification'
      assertProgress(part.progress,{known:partId!=='tree'||![c.from,c.to].includes('cloud'),zero:partId==='zero'})
      for(const [name,bytes] of source) target.set(name,bytes)
      await verifyTree(fixture,p.from,source);await verifyTree(fixture,p.to,target)
      const probe=(await ui.handshake(plan.runId)).probes.find(probe=>probe.id===p.id)
      assert.ok(probe?.consumed,'The real backend must reach the declared finalization checkpoint')
      part.checkpoint=probe;part.verification='INDEPENDENT_MATCH'
    })
  },{id:idFor(c),providers:[...new Set([c.from,c.to])]})
}

import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {createReport} from './report.mjs'
import {cancellationManifest,cancellationProbes,assertCancelledOutput,transferListeners} from './cancellation.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('cancellation declares all boundary directions before writing and representative mid-file/between-file scopes',()=>{
  const report=createReport(plan,kinds,cancellationManifest(plan)),probes=cancellationProbes(plan)
  assert.equal(report.cases.length,13);assert.equal(probes.length,13)
  assert.equal(probes.filter(p=>p.holdPhase==='written').length,1)
  for(const p of probes) {
    ownedPath(plan.targets.map(t=>t.files),p.source);ownedPath(plan.targets.map(t=>t.files),p.target)
    assert.ok(p.holdMs<=5000&&p.holdBytes<=65536)
  }
  assert.ok(report.cases.every(c=>c.status==='NOT_RUN'))
})
test('independent partial-file validation rejects complete, corrupt and unrelated output',()=>{
  assertCancelledOutput('0123456789','0123')
  for(const data of ['','0123456789','abcd','01234567890']) assert.throws(()=>assertCancelledOutput('0123456789',data))
})
test('listener inspection counts live non-enumerable callbacks, without invoking or changing them',async()=>{
  const old=globalThis.window,event='mixed-copy-1-a',listeners=Object.create(null),callbacks=new Map([[42,()=>{}]])
  Object.defineProperty(listeners,event,{value:Object.create(null)})
  Object.defineProperty(listeners[event],'1',{value:{handlerId:42}})
  Object.defineProperty(listeners[event],'2',{value:{handlerId:43}})
  globalThis.window={__internal_unstable_listeners_object_id__:listeners,__TAURI_INTERNALS__:{callbacks}}
  try {
    const ui={browser:{execute:fn=>fn()}}
    assert.deepEqual(await transferListeners(ui),[{event,handler:42}])
    callbacks.delete(42);assert.deepEqual(await transferListeners(ui),[])
  } finally {globalThis.window=old}
})

import assert from 'node:assert/strict'
import {test} from 'node:test'
import {cloudProvider,cloudManifest,cloudLocations} from './cloud-provider.mjs'
import {createReport,recordCase} from './report.mjs'
import {makePlan,validateConfig} from './scope.mjs'
const plan=makePlan(validateConfig({schema:1,targets:{local:'/local/ai_agent_testfolder',usb:null,network:null,mobile:null,cloud:'rclone://Test/ai_agent_testfolder'},rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')

test('real first-available Auto-rename policy uses unreserved suffix one and rejects wrong harness acknowledgement',async()=>{
  const p=cloudLocations(plan),stop=Object.assign(new Error('Stop before private working-copy preparation'),{failureKind:'UNIT_STOP'})
  for(const wrongAcknowledgement of [false,true]) {
    const entries=new Map(),fixture={mkdir:async raw=>entries.set(raw,null),write:async(raw,bytes)=>entries.set(raw,bytes),read:async raw=>entries.get(raw),snapshot:async base=>[...entries].filter(([raw])=>raw.startsWith(base+'/')&&!raw.slice(base.length+1).includes('/')).map(([raw,bytes])=>({path:raw,kind:bytes===null?'dir':'file'}))}
    let clipboard,sourceRoot,workingReached=false
    const report=createReport(plan,['local','cloud'],cloudManifest(plan)),previousWindow=globalThis.window
    globalThis.window={__internal_unstable_listeners_object_id__:{},__TAURI_INTERNALS__:{callbacks:new Map()}}
    const ui={waitTimeout:0,transferTimeout:0,navigate:async()=>{},setView:async()=>{},listing:async()=>{},refresh:async()=>{},
      transfer:async(source,target)=>entries.set(target+'/'+source.split('/').at(-1),entries.get(source)),
      populateClipboard:async(base,paths)=>{sourceRoot=base;clipboard=paths[0]},
      paste:async(target,expected,{conflict})=>{
        assert.equal(sourceRoot,p.conflict)
        if(conflict!=='Auto-rename') return
        const actual=target+'/working-1.txt';entries.set(actual,entries.get(clipboard))
        assert.equal(expected,wrongAcknowledgement?target+'/wrong-acknowledgement.txt':actual,'The UI acknowledgement must name the actual first unreserved output')
      },
      select:async()=>{},handshake:async()=>({cancelTasks:0}),browser:{execute:async(fn,...args)=>fn(...args),waitUntil:async fn=>assert.ok(await fn()),keys:async()=>{workingReached=true;throw stop}}}
    try {
      await assert.rejects(()=>cloudProvider(plan,fixture,ui,(name,action,metadata)=>recordCase(report,metadata,action)),wrongAcknowledgement?/actual first unreserved/:/Stop before private working-copy/)
      const parts=report.cases[0].parts
      assert.equal(parts.find(p=>p.id==='conflict-rename').status,wrongAcknowledgement?'BLOCKED':'PASS')
      assert.equal(workingReached,!wrongAcknowledgement)
      assert.equal(entries.get(p.source),'cloud version A\n');assert.equal(entries.get(p.to+'/working-1.txt'),'cloud version B\n')
    } finally {globalThis.window=previousWindow}
  }
})

test('working-copy follow-up scope declares omitted accepted prefix explicitly instead of marking it PASS',()=>{
  const full=cloudManifest(plan),followup=cloudManifest(plan,'working')
  assert.equal(full.flatMap(c=>c.partIds).length,14);assert.equal(followup.flatMap(c=>c.partIds).length,10)
  for(const id of ['cloud-copy','download','conflict-cancel','conflict-skip']) assert.ok(!followup[0].partIds.includes(id))
  for(const id of ['conflict-rename','working-prepare','working-upload','conflict-overwrite','working-changed-source']) assert.ok(followup[0].partIds.includes(id))
  assert.throws(()=>cloudManifest(plan,'unknown-scope'))
})

import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {createReport} from './report.mjs'
import {progressManifest,progressProbes,assertProgress} from './progress.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('progress declares every boundary direction and three independently verified parts without adopting outside paths',()=>{
  const report=createReport(plan,kinds,progressManifest(plan)),probes=progressProbes(plan)
  assert.equal(report.cases.length,9);assert.equal(probes.length,27)
  for(const kind of kinds.filter(k=>k!=='local')) for(const [a,b] of [['local',kind],[kind,'local']]) {
    assert.ok(report.cases.some(c=>c.id===`progress-${a}-${b}`))
  }
  for(const c of report.cases) {
    assert.deepEqual(c.parts.map(p=>p.id),['file','tree','zero'])
    assert.ok(c.parts.every(p=>p.status==='NOT_RUN'))
  }
  for(const p of probes) {
    ownedPath(plan.targets.map(t=>t.files),p.source);ownedPath(plan.targets.map(t=>t.files),p.target)
    assert.ok(p.holdMs<=5000&&p.slowMs<=500)
  }
})
test('progress evidence rejects fabricated bytes, premature completion, absent activity and stale activity',()=>{
  const active={visible:true,active:true,label:'Copying…',detail:'64.0 KB / 64.0 KB',percent:'100'}
  const idle={visible:false,active:false,label:null,detail:null,percent:null}
  assertProgress([active,idle],{known:true})
  for(const bad of [
    [{...active,detail:'1 B / 1 B'},idle],
    [{...active,label:'Copy completed'},idle],
    [idle],
    [active],
    [{...active,detail:null,percent:null},idle],
  ]) assert.throws(()=>assertProgress(bad,{known:true}))
  assertProgress([{...active,detail:null,percent:null},idle],{known:false})
  assertProgress([{...active,detail:null,percent:null},idle],{known:true,zero:true})
})

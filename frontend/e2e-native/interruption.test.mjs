import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {createReport} from './report.mjs'
import {interruptionManifest,interruptionProbes} from './interruption.mjs'
const config=validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'})
const uuid='23456789-1234-4234-9234-123456789abc',full=makePlan(config,uuid),
  local=makePlan({...config,targets:config.targets.filter(t=>t.kind==='local'),rcloneConfig:null},uuid)
test('process interruption refuses provider plans and requires separate interruption/startup/diagnostic evidence',()=>{
  assert.throws(()=>interruptionManifest(full),/local-only/)
  assert.throws(()=>interruptionProbes(full),/local-only/)
  const report=createReport(local,kinds,interruptionManifest(local)),probes=interruptionProbes(local)
  assert.equal(report.cases.length,1);assert.deepEqual(report.cases[0].parts.map(p=>p.id),
    ['interrupted-overwrite','safe-startup','recovery-diagnostics'])
  assert.equal(report.cases[0].status,'NOT_RUN');assert.equal(probes.length,1)
  ownedPath([local.targets[0].files],probes[0].source);ownedPath([local.targets[0].files],probes[0].target)
  assert.equal(probes[0].holdPhase,'written');assert.equal(probes[0].holdBytes,16384)
  assert.ok(probes[0].holdMs<=5000)
})

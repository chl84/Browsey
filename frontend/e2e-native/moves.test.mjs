import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {createReport} from './report.mjs'
import {moveManifest,moveProbes,moveFaults} from './moves.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('move scope includes all providers within/across boundaries and genuine pre-delete stops',()=>{
  const report=createReport(plan,kinds,moveManifest(plan)),probes=moveProbes(plan),faults=moveFaults(plan)
  assert.equal(report.cases.length,22);assert.equal(probes.length,20);assert.equal(faults.length,1)
  for(const k of kinds) assert.ok(report.cases.some(c=>c.id===`moves-${k}-${k}-before`))
  assert.equal(probes.filter(p=>p.holdPhase==='finalize').length,3)
  assert.equal(probes.filter(p=>p.holdPhase==='written').length,1)
  for(const p of probes) {ownedPath(plan.targets.map(t=>t.files),p.source);ownedPath(plan.targets.map(t=>t.files),p.target)}
  ownedPath(plan.targets.map(t=>t.files),faults[0].source)
  assert.ok(report.cases.every(c=>c.status==='NOT_RUN'))
})

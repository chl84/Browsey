import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {createReport} from './report.mjs'
import {ioFaultManifest,ioFaultProbes} from './iofaults.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('I/O faults declare exact bounded paths and separate pre-dispatch cancellation from failures',()=>{
  const report=createReport(plan,kinds,ioFaultManifest(plan)),probes=ioFaultProbes(plan)
  assert.equal(report.cases.length,9);assert.equal(probes.length,9)
  assert.equal(probes.filter(p=>p.holdPhase==='prepare'&&!p.fault).length,4)
  assert.deepEqual([...new Set(probes.map(p=>p.fault).filter(Boolean))].sort(),['no-space','transient','unavailable'])
  for(const p of probes) {
    ownedPath(plan.targets.map(t=>t.files),p.source);ownedPath(plan.targets.map(t=>t.files),p.target)
    assert.ok(p.holdMs<=5000&&p.holdBytes<=65536&&p.slowMs<=500)
  }
  assert.ok(report.cases.every(c=>c.status==='NOT_RUN'))
  for(const c of report.cases) for(const caps of Object.values(c.requirements)) {
    assert.equal(caps.includes('preparation-cancellation'),c.id.includes('-preparing-'))
    assert.equal(caps.includes('bounded-owned-io-fault'),!c.id.includes('-preparing-'))
  }
})

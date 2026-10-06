import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds} from './scope.mjs'
import {createReport} from './report.mjs'
import {usbManifest} from './providers.mjs'
const config=selected=>validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,selected.includes(k)?k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`:null])),rcloneConfig:selected.includes('cloud')?'/local/ai_agent_testfolder/rclone.conf':null})
test('USB acceptance requires a real selected USB, declares Unix access separately and refuses other providers',()=>{
  const plan=makePlan(config(['local','usb']),'23456789-1234-4234-9234-123456789abc')
  const report=createReport(plan,kinds,usbManifest(plan))
  assert.deepEqual(report.cases.find(c=>c.id==='access-usb').providers,['usb'])
  assert.ok(!report.cases.some(c=>c.id==='access-local'))
  assert.ok(report.cases.some(c=>c.id==='copy-local-usb'))
  assert.ok(report.cases.some(c=>c.id==='move-usb-local'))
  for(const selected of [['local'],['local','usb','mobile']]) assert.throws(()=>usbManifest(makePlan(config(selected),plan.runId)))
})

test('network acceptance declares bounded large deletion and both progress/Cancel routes',async()=>{
  const {networkManifest,networkProbes}=await import('./providers.mjs')
  const plan=makePlan(config(['local','network']),'23456789-1234-4234-9234-123456789abc')
  const report=createReport(plan,kinds,networkManifest(plan))
  assert.deepEqual(report.cases.find(c=>c.id==='provider-network').parts.map(p=>p.id),['large-delete-cancel','large-delete-confirm','stale-rename','fresh-metadata'])
  for(const direction of ['local-network','network-local']) {
    assert.ok(report.cases.some(c=>c.id===`progress-${direction}`))
    assert.ok(report.cases.some(c=>c.id===`cancel-${direction}-before`))
  }
  assert.ok(networkProbes(plan).every(p=>p.holdMs<=5000&&p.holdBytes<=65536))
  assert.throws(()=>networkManifest(makePlan(config(['local','network','mobile']),plan.runId)))
})

test('MTP acceptance declares thumbnail/metadata behavior plus source preservation after provider name failure',async()=>{
  const {mobileManifest}=await import('./providers.mjs'),plan=makePlan(config(['local','mobile']),'23456789-1234-4234-9234-123456789abc')
  const report=createReport(plan,kinds,mobileManifest(plan))
  assert.deepEqual(report.cases.find(c=>c.id==='provider-mobile').providers,['mobile'])
  assert.ok(report.cases.find(c=>c.id==='names-mobile').parts.some(p=>p.id==='quoted-name-rejection'))
  assert.throws(()=>mobileManifest(makePlan(config(['local','mobile','usb']),plan.runId)))
})

test('cloud provider acceptance binds working-copy source and candidate error probes to the selected generated paths',async()=>{
  const {cloudManifest,cloudProbes,cloudLocations}=await import('./cloud-provider.mjs'),plan=makePlan(config(['local','cloud']),'23456789-1234-4234-9234-123456789abc')
  const report=createReport(plan,kinds,cloudManifest(plan));assert.equal(report.cases.flatMap(c=>c.parts).length,14)
  const probes=cloudProbes(plan),p=cloudLocations(plan);assert.equal(probes.length,3)
  assert.ok(probes.every(probe=>probe.source.startsWith(p.errors+'/')&&probe.target.startsWith(p.to+'/')&&probe.holdPhase==='validation'&&probe.holdBytes===0))
  assert.ok(p.source.startsWith(p.to+'/'))
  assert.throws(()=>cloudManifest(makePlan(config(['local','cloud','mobile']),plan.runId)))
})

import assert from 'node:assert/strict'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds} from './scope.mjs'
import {createReport} from './report.mjs'
import {usbManifest} from './providers.mjs'
const config=selected=>validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,selected.includes(k)?`/${k}/ai_agent_testfolder`:null])),rcloneConfig:null})
test('USB acceptance requires a real selected USB, declares Unix access separately and refuses other providers',()=>{
  const plan=makePlan(config(['local','usb']),'23456789-1234-4234-9234-123456789abc')
  const report=createReport(plan,kinds,usbManifest(plan))
  assert.deepEqual(report.cases.find(c=>c.id==='access-usb').providers,['usb'])
  assert.ok(!report.cases.some(c=>c.id==='access-local'))
  assert.ok(report.cases.some(c=>c.id==='copy-local-usb'))
  assert.ok(report.cases.some(c=>c.id==='move-usb-local'))
  for(const selected of [['local'],['local','usb','mobile']]) assert.throws(()=>usbManifest(makePlan(config(selected),plan.runId)))
})

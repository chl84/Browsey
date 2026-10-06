import assert from 'node:assert/strict'
import { test } from 'node:test'
import { guards, guardManifest } from './guards.mjs'
import { makePlan, validateConfig, kinds, ownedPath } from './scope.mjs'
import { createReport, recordCase, finishReport } from './report.mjs'
const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind => [kind,
  kind === 'cloud' ? 'rclone://test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
  rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '23456789-1234-4234-9234-123456789abc')
test('unsafe-transfer manifest declares exact same-target, descendant, ancestor and unique-copy scope plus cloud aliases', () => {
  const report=createReport(plan,kinds,guardManifest(plan))
  assert.equal(report.cases.length,5);assert.equal(report.cases.reduce((n,c)=>n+c.parts.length,0),47)
  for(const c of report.cases) for(const id of ['cut-same-file','cut-same-directory','copy-descendant','move-descendant','copy-into-self','move-into-self','copy-ancestor','move-ancestor','same-folder-copy-new-name']) {
    assert.ok(c.parts.some(p=>p.id===id))
  }
  assert.equal(report.cases.filter(c=>c.parts.some(p=>p.id.includes('casefold'))).length,1)
})
test('unsafe transfer verifier detects source/ancestor damage instead of accepting rejection text as preservation', async()=>{
  for(const damage of [false,true]) {
    const items=new Map(),roots=plan.targets.map(t=>t.files)
    const fixture={mkdir:async path=>{ownedPath(roots,path);items.set(path,null)},write:async(path,bytes)=>{ownedPath(roots,path);items.set(path,bytes)},
      read:async path=>items.get(path),snapshot:async base=>[...items].filter(([p])=>p.slice(0,p.lastIndexOf('/'))===base).map(([path,bytes])=>({path,kind:bytes===null?'dir':'file'}))}
    let selected,calls=0
    const ui={setView:async()=>{},navigate:async()=>{},enterPath:async()=>{},
      populateClipboard:async(_base,paths)=>{selected=paths},paste:async(_dest,first,{expectedError})=>{
        calls++;if(expectedError){if(damage) items.set(selected[0], 'damaged source')}
        else items.set(first,items.get(selected[0]))
      }}
    const report=createReport(plan,kinds,guardManifest(plan));const run=guards(plan,fixture,ui,(_n,action,metadata)=>recordCase(report,metadata,action))
    if(damage){await assert.rejects(run,/Exact generated paths|Generated file bytes/);finishReport(report,Error('Source damaged'));assert.equal(calls,1);assert.equal(report.status,'FAIL');assert.ok(report.cases.some(c=>c.status==='NOT_RUN'))}
    else {await run;finishReport(report);assert.equal(report.status,'PASS');assert.equal(calls,47)}
  }
})

test('scoped guard follow-up declares only both cloud aliases and all mobile parts', () => {
  const manifest = guardManifest(plan, 'remaining')
  assert.deepEqual(manifest.map(item => item.providers[0]), ['cloud', 'mobile'])
  assert.deepEqual(manifest[0].partIds, ['copy-casefold-descendant', 'move-casefold-descendant'])
  assert.equal(manifest[1].partIds.length, 9)
})

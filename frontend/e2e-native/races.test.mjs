import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {createReport} from './report.mjs'
import {raceManifest,raceProbes,ownedWriter,ownedSymlinkSwap} from './races.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('writer races declare six bounded local/USB/cloud cases, not universal atomicity',()=>{
  const report=createReport(plan,kinds,raceManifest(plan)),probes=raceProbes(plan)
  assert.equal(report.cases.length,6);assert.equal(probes.length,6)
  assert.equal(probes.filter(p=>p.holdPhase==='written').length,4)
  for(const p of probes) {ownedPath(plan.targets.map(t=>t.files),p.source);ownedPath(plan.targets.map(t=>t.files),p.target);assert.ok(p.holdMs<=5000)}
  assert.ok(report.cases.every(c=>c.status==='NOT_RUN'))
})
test('controlled writer rejects outside, oversized and replaced files without touching the replacement',async()=>{
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-native-writer-policy-'))
  try {
    const file=path.join(root,'body.txt');await fs.writeFile(file,'old-generated',{mode:0o600})
    await assert.rejects(()=>ownedWriter([root],'/outside/body.txt'))
    const writer=await ownedWriter([root],file)
    try {
      await assert.rejects(()=>writer.write('x'.repeat(65537)))
      assert.equal(await fs.readFile(file,'utf8'),'old-generated')
      await writer.write('new-generated');assert.equal(await fs.readFile(file,'utf8'),'new-generated')
      await fs.rename(file,path.join(root,'saved.txt'));await fs.writeFile(file,'foreign',{mode:0o600})
      await assert.rejects(()=>writer.write('do-not-overwrite'),/inode changed/)
      assert.equal(await fs.readFile(file,'utf8'),'foreign')
    } finally {await writer.handle.close()}
  } finally {await fs.rm(root,{recursive:true,force:true})}
})
test('owned symlink swap restores only its own link and refuses an outside referent',async()=>{
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-native-swap-policy-'))
  try {
    const file=path.join(root,'body.txt'),referent=path.join(root,'sentinel.txt')
    await fs.writeFile(file,'original',{mode:0o600});await fs.writeFile(referent,'sentinel',{mode:0o600})
    await assert.rejects(()=>ownedSymlinkSwap([root],file,'/outside/sentinel.txt'))
    const restore=await ownedSymlinkSwap([root],file,referent)
    assert.ok((await fs.lstat(file)).isSymbolicLink());await restore()
    assert.equal(await fs.readFile(file,'utf8'),'original');assert.equal(await fs.readFile(referent,'utf8'),'sentinel')
    const refused=await ownedSymlinkSwap([root],file,referent)
    await fs.unlink(file);await fs.writeFile(file,'foreign',{mode:0o600})
    await assert.rejects(refused,/Owned link changed/)
    assert.equal(await fs.readFile(file,'utf8'),'foreign');assert.equal(await fs.readFile(path.join(root,'source.saved'),'utf8'),'original')
  } finally {await fs.rm(root,{recursive:true,force:true})}
})

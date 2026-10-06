import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds} from './scope.mjs'
import {createReport} from './report.mjs'
import {accessManifest,ownedMode} from './access.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('access acceptance separates real local mode enforcement from all-provider capability inspection',()=>{
  const report=createReport(plan,kinds,accessManifest(plan))
  assert.equal(report.cases.length,6);assert.equal(report.cases.flatMap(c=>c.parts).length,18)
  assert.ok(report.cases.every(c=>c.status==='NOT_RUN'))
  assert.deepEqual(report.cases.find(c=>c.id==='access-local').providers,['local'])
})
test('owned permission changes stay on their captured inode and refuse outside paths and unsupported modes',async()=>{
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-native-access-policy-'))
  try {
    const file=path.join(root,'body.txt');await fs.writeFile(file,'generated',{mode:0o600})
    const restore=await ownedMode([root],file,0o400);assert.equal((await fs.lstat(file)).mode&0o777,0o400)
    await restore();assert.equal((await fs.lstat(file)).mode&0o777,0o600)
    await assert.rejects(()=>ownedMode([root],'/outside/body.txt',0o400))
    await assert.rejects(()=>ownedMode([root],file,0o777))
    const restoreAgain=await ownedMode([root],file,0o400)
    await fs.rename(file,path.join(root,'saved.txt'));await fs.writeFile(file,'foreign',{mode:0o600})
    await assert.rejects(restoreAgain,/inode changed/)
    assert.equal((await fs.lstat(file)).mode&0o777,0o600)
    assert.equal(await fs.readFile(file,'utf8'),'foreign')
  } finally {await fs.rm(root,{recursive:true,force:true})}
})

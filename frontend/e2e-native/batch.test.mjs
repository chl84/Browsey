import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { batchManifest, batchFaults, denyOwnedRead } from './batch.mjs'
import { makePlan, validateConfig, kinds, ownedPath } from './scope.mjs'
const plan = makePlan(validateConfig({ schema: 1, targets: Object.fromEntries(kinds.map(kind => [kind,
  kind === 'cloud' ? 'rclone://test/ai_agent_testfolder' : `/${kind}/ai_agent_testfolder`])),
  rcloneConfig: '/local/ai_agent_testfolder/rclone.conf' }), '23456789-1234-4234-9234-123456789abc')
test('batch cases distinguish genuine local read failures from bounded cloud/refresh injection and declare exact fault paths',()=>{
  const cases=batchManifest(plan),faults=batchFaults(plan),roots=plan.targets.map(t=>t.files)
  assert.equal(cases.length,8);assert.equal(faults.length,4);assert.equal(new Set(faults.map(f=>f.id)).size,4)
  for(const f of faults){assert.ok(cases.some(c=>c.id===f.id));for(const p of [f.source,f.refreshTarget,f.armSource].filter(Boolean))ownedPath(roots,p)}
  assert.equal(faults.filter(f=>f.source).length,2);assert.equal(faults.filter(f=>f.refreshTarget&&f.armSource).length,2)
  assert.ok(!faults.some(f=>f.id==='batch-upload-copy'||f.id==='batch-local-copy'))
})
test('read-denial fixture refuses arbitrary/link/inherited-mode files and restores only the held owned inode',async()=>{
  const base=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-batch-policy-'));await fs.chmod(base,0o700)
  const file=path.join(base,'new.txt'),link=path.join(base,'link.txt')
  try {await fs.writeFile(file,'generated',{mode:0o600});await fs.symlink(file,link)
    await assert.rejects(denyOwnedRead([base],'/unapproved/file'))
    await assert.rejects(denyOwnedRead([base],link))
    await fs.chmod(file,0o644);await assert.rejects(denyOwnedRead([base],file));assert.equal((await fs.stat(file)).mode&0o777,0o644)
    await fs.chmod(file,0o600);const restore=await denyOwnedRead([base],file);assert.equal((await fs.stat(file)).mode&0o777,0)
    await restore();assert.equal((await fs.stat(file)).mode&0o777,0o600);assert.equal(await fs.readFile(file,'utf8'),'generated')
    const restore2=await denyOwnedRead([base],file);await fs.rename(file,path.join(base,'retained.txt'));await fs.writeFile(file,'replacement',{mode:0o600})
    await assert.rejects(restore2(),/inode changed/);assert.equal(await fs.readFile(file,'utf8'),'replacement')
    await fs.chmod(path.join(base,'retained.txt'),0o600)
  } finally {await fs.rm(base,{recursive:true,force:true})}
})

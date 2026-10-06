import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {test} from 'node:test'
import {networkDeleteFixture,undoMetadata} from './provider-fixtures.mjs'
test('large-delete writer refuses other providers/outside scope and never overwrites an existing generated file',async()=>{
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-provider-fixture-'))
  try {
    const plan={targets:[{kind:'network',files:root}]},file=path.join(root,'large-delete.bin')
    await fs.writeFile(file,'keep original',{mode:0o600})
    await assert.rejects(()=>networkDeleteFixture(plan,file),e=>e.code==='EEXIST')
    assert.equal(await fs.readFile(file,'utf8'),'keep original')
    await assert.rejects(()=>networkDeleteFixture(plan,'/outside/large-delete.bin'))
    await assert.rejects(()=>networkDeleteFixture({targets:[{kind:'local',files:root}]},file))
    await assert.rejects(()=>networkDeleteFixture(plan,path.join(root,'other.bin')))
  } finally {await fs.rm(root,{recursive:true,force:true})}
})
test('large fixture retains partial data and closes without retry on zero-byte writes or elapsed limit',async()=>{
  const plan={targets:[{kind:'network',files:'/owned'}]}
  for(const timeout of [false,true]) {
    let writes=0,closed=0,clock=0
    const filesystem={lstat:async()=>{throw Object.assign(new Error(),{code:'ENOENT'})},open:async()=>({write:async()=>{writes++;return {bytesWritten:0}},close:async()=>{closed++}})}
    await assert.rejects(()=>networkDeleteFixture(plan,'/owned/large-delete.bin',{filesystem,now:()=>timeout?clock++*60_001:0}))
    assert.equal(writes,timeout?0:1);assert.equal(closed,1)
  }
})
test('undo absence proof checks metadata without opening backup contents, including a large backup',async()=>{
  const local={kind:'local',run:'/owned/run'},plan={targets:[local]},root='/owned/run/profile/data/browsey/undo-sessions'
  const stat=directory=>({uid:process.getuid(),nlink:1,size:directory?0:32*1024*1024,isDirectory:()=>directory,isFile:()=>!directory,isSymbolicLink:()=>false})
  const filesystem={lstat:async p=>stat(p!==root+'/backup.bin'),readdir:async()=>['backup.bin'],open:async()=>{throw Error('Content reads forbidden')}}
  const result=await undoMetadata(plan,{filesystem});assert.equal(result.find(x=>x.kind==='file').size,32*1024*1024)
})

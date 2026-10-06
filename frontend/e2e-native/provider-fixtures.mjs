import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {constants} from 'node:fs'
import {child,noLinks,ownedPath,inside} from './scope.mjs'

// A dedicated deletion fixture; the normal fixture writer remains capped at 64 KiB.
export async function networkDeleteFixture(plan,raw,{filesystem=fs,now=Date.now}={}) {
  const target=plan.targets.find(t=>t.kind==='network')
  assert.ok(target&&inside(target.files,raw),'Large deletion data requires the selected owned network subtree')
  ownedPath([target.files],raw);assert.ok(raw.endsWith('/large-delete.bin'))
  await noLinks(raw,filesystem)
  const handle=await filesystem.open(raw,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600)
  const bytes=32*1024*1024,block=Buffer.alloc(65536,0x5a),started=now()
  try {
    for(let offset=0;offset<bytes;) {
      assert.ok(now()-started<=60_000,'Bounded network fixture write exceeded 60 seconds; partial data retained')
      const written=await handle.write(block,0,Math.min(block.length,bytes-offset),null)
      assert.ok(written.bytesWritten>0&&written.bytesWritten<=block.length);offset+=written.bytesWritten
    }
    const stat=await handle.stat();assert.ok(stat.isFile()&&stat.nlink===1);assert.equal(stat.size,bytes)
    return {bytes,blockBytes:block.length,elapsedMs:now()-started,contentReads:0}
  } finally {await handle.close()}
}

// Metadata only: never download any backup in order to prove its absence.
export async function undoMetadata(plan,{filesystem=fs}={}) {
  const local=plan.targets.find(t=>t.kind==='local'),root=['profile','data','browsey','undo-sessions'].reduce(child,local.run)
  const entries=[]
  async function visit(raw,depth) {
    assert.ok(inside(root,raw)&&depth<=8&&entries.length<256,'Private undo metadata budget exceeded')
    await noLinks(raw,filesystem)
    const stat=await filesystem.lstat(raw);assert.equal(stat.uid,process.getuid());assert.ok(stat.isDirectory()||stat.isFile())
    if(stat.isFile()) assert.equal(stat.nlink,1)
    entries.push({relative:raw.slice(root.length),kind:stat.isDirectory()?'dir':'file',size:stat.isDirectory()?0:stat.size})
    if(stat.isDirectory()) {const names=await filesystem.readdir(raw);assert.ok(names.length<=128);for(const name of names) await visit(child(raw,name),depth+1)}
  }
  try {await filesystem.lstat(root)} catch(error) {if(error.code==='ENOENT') return [];throw error}
  await visit(root,0);return entries.sort((a,b)=>a.relative.localeCompare(b.relative))
}

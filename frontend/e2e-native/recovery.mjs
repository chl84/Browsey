import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {child,noLinks,inside} from './scope.mjs'
import {regularFile} from './fixtures.mjs'

// Only this candidate's generated undo store; never inspect other profile data.
export async function recoveryEvidence(plan,original,{filesystem=fs,read=regularFile}={}) {
  const local=plan.targets.find(t=>t.kind==='local'),root=['profile','data','browsey','undo-sessions'].reduce(child,local.run)
  const evidence={files:0,bytes:0,originalCopies:0,recoveryMarkers:0}
  let entries=0
  const visit=async(raw,depth)=>{
    assert.ok(inside(root,raw)&&depth<=6&&++entries<=256&&evidence.files<128,'Bounded private recovery scope exceeded')
    await noLinks(raw,filesystem)
    const stat=await filesystem.lstat(raw)
    assert.equal(stat.uid,process.getuid(),'Recovery must belong to the owned candidate')
    if(stat.isDirectory()) {
      assert.equal(stat.mode&0o777,0o700)
      const children=await filesystem.readdir(raw);assert.ok(children.length<=128)
      for(const name of children) await visit(child(raw,name),depth+1)
    } else {
      assert.ok(stat.isFile()&&stat.nlink===1&&stat.size<=65536,'Expected bounded generated recovery data')
      evidence.files++;evidence.bytes+=stat.size;assert.ok(evidence.bytes<=1024*1024)
      const text=(await read(raw)).text
      if(text===original) evidence.originalCopies++
      if(raw.endsWith('.recovery-required')) {
        assert.match(text,/^Browsey interrupted\/failed file operation\. Recover this backup manually/)
        assert.match(text,/Destination: .+\nBackup: .+/);evidence.recoveryMarkers++
      }
    }
  }
  try {await filesystem.lstat(root)} catch(error) {if(error.code==='ENOENT') return evidence;throw error}
  await visit(root,0);return evidence
}

import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {test} from 'node:test'
import {makePlan,validateConfig,kinds,ownedPath} from './scope.mjs'
import {overwriteManifest,overwriteProbes} from './overwrite.mjs'
import {recoveryEvidence} from './recovery.mjs'
const plan=makePlan(validateConfig({schema:1,targets:Object.fromEntries(kinds.map(k=>[k,
  k==='cloud'?'rclone://test/ai_agent_testfolder':`/${k}/ai_agent_testfolder`])),
  rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'23456789-1234-4234-9234-123456789abc')
test('overwrite scope declares local/USB partial cancellation and representative cloud pre-write refusals',()=>{
  assert.equal(overwriteManifest(plan).length,9)
  const probes=overwriteProbes(plan);assert.equal(probes.length,6)
  for(const p of probes) {ownedPath(plan.targets.map(t=>t.files),p.source);ownedPath(plan.targets.map(t=>t.files),p.target)}
  assert.equal(probes.filter(p=>p.holdPhase==='written').length,2)
})
test('private recovery inspection verifies exact original bytes and real diagnostics without reading credentials',async()=>{
  const run=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-native-recovery-policy-'))
  try {
    const root=path.join(run,'profile/data/browsey/undo-sessions'),bucket=path.join(root,'session-test','bucket')
    await fs.mkdir(bucket,{recursive:true,mode:0o700})
    await fs.writeFile(path.join(bucket,'body.txt'),'original bytes',{mode:0o600})
    await fs.writeFile(path.join(root,'session-test','bucket.recovery-required'),
      'Browsey interrupted/failed file operation. Recover this backup manually before deleting the session.\nDestination: "owned target"\nBackup: "owned backup"\n',{mode:0o600})
    await fs.writeFile(path.join(run,'profile','do-not-read.conf'),'private',{mode:0o600})
    const p={targets:[{kind:'local',run}]}
    const e=await recoveryEvidence(p,'original bytes');assert.equal(e.originalCopies,1);assert.equal(e.recoveryMarkers,1)
    assert.equal(e.files,2)
    assert.equal((await recoveryEvidence(p,'different bytes')).originalCopies,0)
    await fs.symlink(path.join(run,'profile','do-not-read.conf'),path.join(bucket,'unsafe'))
    await assert.rejects(()=>recoveryEvidence(p,'original bytes'),/symlink/i)
  } finally {await fs.rm(run,{recursive:true,force:true})}
})

import assert from 'node:assert/strict'
import {test} from 'node:test'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {makePlan,validateConfig} from './scope.mjs'
import {cloudExportManifest} from './cloud-export.mjs'
import {cloudScaleManifest,largeOwnedFixture,boundedFileDigest} from './cloud-scale.mjs'
import {cloudTrashManifest,cloudTrashLocations} from './cloud-trash.mjs'
import {cloudRaceManifest,cloudRaceProbes} from './cloud-race.mjs'
import {acceptanceRows} from './acceptance-map.mjs'
function planFor(local='/generated/ai_agent_testfolder') {return makePlan(validateConfig({schema:1,targets:{local,cloud:'rclone://Test/ai_agent_testfolder'},rcloneConfig:local+'/rclone.conf'}),'00000000-0000-4000-8000-000000000000')}
test('cloud follow-ups require explicit local/cloud scope and declare mapped native parts',()=>{
  const plan=planFor()
  for(const manifest of [cloudExportManifest,cloudScaleManifest,cloudRaceManifest,cloudTrashManifest]) {
    for(const c of manifest(plan)){assert.deepEqual(c.providers,['local','cloud']);assert.ok(c.partIds.length);assert.ok(acceptanceRows(c.id).length)}
    assert.throws(()=>manifest({...plan,targets:[plan.targets[0]]}))
  }
  const trash=cloudTrashLocations(plan);assert.equal(trash.name,`browsey-web-restore-${plan.runId}.txt`);assert.ok(trash.target.startsWith(plan.targets[1].files+'/cloud-trash-source/'))
  const probe=cloudRaceProbes(plan)[0];assert.equal(probe.holdPhase,'finalize');assert.equal(probe.holdMs,5000);assert.ok(probe.source.startsWith(plan.targets[0].files+'/'));assert.ok(probe.target.startsWith(plan.targets[1].files+'/'))
})
test('larger fixtures retain ordinary writer limits and reject outside/link/oversize inputs',async t=>{
  const temporary=await fs.mkdtemp(path.join(os.tmpdir(),'b'));t.after(()=>fs.rm(temporary,{recursive:true,force:true}));const local=path.join(temporary,'ai_agent_testfolder'),plan=planFor(local),root=plan.targets[0].files,source=root+'/cloud-scale-source'
  await fs.mkdir(source,{recursive:true,mode:0o700})
  for(const [raw,size]of [[temporary+'/outside.bin',131072],[root+'/other/large-128k.bin',131072],[source+'/unexpected.bin',131072],[source+'/large-128k.bin',16777217]])await assert.rejects(largeOwnedFixture(plan,raw,size))
  const file=source+'/large-128k.bin',before=await largeOwnedFixture(plan,file,131072);assert.equal(before.bytes,131072);assert.deepEqual(await boundedFileDigest([root],file),before)
  const alias=source+'/large-512k.bin';await fs.symlink(file,alias);await assert.rejects(largeOwnedFixture(plan,alias,524288));await assert.rejects(boundedFileDigest([root],alias));await assert.rejects(boundedFileDigest([root],temporary+'/outside.bin'))
  assert.deepEqual(await boundedFileDigest([root],file),before)
})

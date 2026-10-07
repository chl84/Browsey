import assert from 'node:assert/strict'
import {test} from 'node:test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import {spawnSync} from 'node:child_process'
import {generatedStorageImage,storageManifest} from './storage-performance.mjs'
import {makePlan,validateConfig} from './scope.mjs'

test('storage measurements preserve provider scope and reject unrelated roots',()=>{
  const plan=makePlan(validateConfig({schema:1,targets:{local:'/local/ai_agent_testfolder',usb:'/usb/ai_agent_testfolder',mobile:'/mobile/ai_agent_testfolder',cloud:'rclone://Test/ai_agent_testfolder'},rcloneConfig:'/local/ai_agent_testfolder/rclone.conf'}),'00000000-0000-4000-8000-000000000000')
  assert.deepEqual(storageManifest(plan).map(x=>x.providers),[['local'],['usb'],['cloud'],['mobile']])
  assert.ok(storageManifest(plan).every(x=>x.partIds.length===6))
  assert.throws(()=>storageManifest({...plan,targets:[...plan.targets,{kind:'network'}]}))
  const image=generatedStorageImage(0);assert.ok(image.length<65536);assert.equal(image.readUInt32BE(16),1280);assert.equal(image.readUInt32BE(20),720)
  assert.notDeepEqual(image,generatedStorageImage(1));assert.throws(()=>generatedStorageImage(60))
})
test('file-data cache helper rejects outside aliases and inspects only exact generated files',t=>{
  const temporary=fs.mkdtempSync(path.join(os.tmpdir(),'browsey-cache-policy-'));t.after(()=>fs.rmSync(temporary,{recursive:true,force:true}))
  const root=path.join(temporary,'ai_agent_testfolder/.bnt-00000000000040008000000000000000/files'),base=path.join(root,'storage-sample-0')
  fs.mkdirSync(base,{recursive:true});const file=path.join(base,'image-00.png');fs.writeFileSync(file,generatedStorageImage(0),{mode:0o600})
  const helper=new URL('../../tests/support/native_fixture_cache.py',import.meta.url).pathname
  const check=paths=>spawnSync('/usr/bin/python3',['-B',helper],{input:JSON.stringify({root,paths}),encoding:'utf8',timeout:10_000,maxBuffer:65536})
  const result=check([file]);assert.equal(result.status,0,result.stderr);const evidence=JSON.parse(result.stdout);assert.equal(evidence.files.length,1);assert.equal(evidence.files[0].bytes,fs.statSync(file).size);assert.equal(evidence.verifiedDataCold,evidence.files[0].residentAfter===0)
  assert.ok(evidence.files[0].residentAfter>=0&&evidence.files[0].residentAfter<=evidence.files[0].pages)
  for(const paths of [[path.join(temporary,'outside')],[file,file],[base+'/../image-00.png'],[path.join(root,'other/image-00.png')]])assert.notEqual(check(paths).status,0)
  const alias=path.join(base,'image-01.png');fs.symlinkSync(file,alias);assert.notEqual(check([alias]).status,0)
})

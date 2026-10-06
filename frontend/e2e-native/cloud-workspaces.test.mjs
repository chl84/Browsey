import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {test} from 'node:test'
import {workingCopies,editWorkingCopy} from './cloud-workspaces.mjs'
test('private working-copy edits refuse foreign manifests, changed originals and hard-link aliases without truncating data',async()=>{
  const run=await fs.mkdtemp(path.join(os.tmpdir(),'browsey-cloud-workspace-')),source='rclone://Test/owned/working.txt',plan={targets:[{kind:'local',run}]}
  const dir=path.join(run,'profile/data/browsey/cloud-workspaces/123-ab/files'),file=path.join(dir,'working.txt'),manifest=path.join(dir,'../manifest.json')
  try {
    await fs.mkdir(dir,{recursive:true,mode:0o700});await fs.writeFile(file,'original',{mode:0o600})
    const save=async(sourcePath=source,localPath=file)=>fs.writeFile(manifest,JSON.stringify({id:'123-ab',sourcePath,localPath}),{mode:0o600})
    await save();await assert.rejects(()=>editWorkingCopy(plan,source,'123-ab','wrong original','changes'));assert.equal(await fs.readFile(file,'utf8'),'original')
    await save('rclone://Outside/personal.txt');await assert.rejects(()=>workingCopies(plan,source))
    await save(source,'/outside/file');await assert.rejects(()=>workingCopies(plan,source))
    await save();await fs.link(file,path.join(dir,'alias'));await assert.rejects(()=>workingCopies(plan,source),/hard links/);await fs.unlink(path.join(dir,'alias'))
    await editWorkingCopy(plan,source,'123-ab','original','generated edits');assert.equal(await fs.readFile(file,'utf8'),'generated edits')
  } finally {await fs.rm(run,{recursive:true,force:true})}
})

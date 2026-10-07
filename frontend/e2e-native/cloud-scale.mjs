import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {constants} from 'node:fs'
import {createHash} from 'node:crypto'
import {execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {performance} from 'node:perf_hooks'
import {child,ownedPath,noLinks,inside} from './scope.mjs'
import {recordPart} from './report.mjs'
import {rclonePath} from './fixtures.mjs'
import {releasedResources} from './resources.mjs'
import {measuredSamples} from './metrics.mjs'
const exec=promisify(execFile)
const byteLimit=16*1024*1024
export const cloudScaleManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local','cloud']);return [{id:'provider-cloud-scale',name:'Bounded OneDrive mixed-size tree',providers:['local','cloud'],partIds:['generated-tree','upload','remote-digests','download','preserved-trees']}]}
export async function boundedFileDigest(roots,raw) {
  ownedPath(roots,raw);await noLinks(raw,fs)
  const handle=await fs.open(raw,constants.O_RDONLY|constants.O_NOFOLLOW)
  try {
    const stat=await handle.stat();assert.ok(stat.isFile()&&stat.uid===process.getuid()&&stat.nlink===1&&stat.size<=byteLimit)
    const hash=createHash('sha256');let bytes=0
    for await(const chunk of handle.createReadStream({autoClose:false})){bytes+=chunk.length;assert.ok(bytes<=byteLimit);hash.update(chunk)}
    const end=await handle.stat();assert.equal(end.size,stat.size);assert.equal(end.mtimeMs,stat.mtimeMs);assert.equal(bytes,stat.size)
    return {bytes,sha256:hash.digest('hex')}
  } finally {await handle.close()}
}
export async function largeOwnedFixture(plan,raw,bytes) {
  const local=plan.targets.find(t=>t.kind==='local');ownedPath([local.files],raw)
  const allowed={'large-128k.bin':128*1024,'large-512k.bin':512*1024,'large-2m.bin':2*1024*1024,'large-8m.bin':8*1024*1024}
  const base=child(local.files,'cloud-scale-source'),name=raw.slice(base.length+1)
  assert.ok(inside(base,raw)&&Object.hasOwn(allowed,name)&&allowed[name]===bytes);await noLinks(raw,fs)
  const handle=await fs.open(raw,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600),block=Buffer.alloc(65536)
  for(let i=0;i<block.length;i++)block[i]=(i*31+(i>>>8)*17)%256
  const started=performance.now()
  try {for(let offset=0;offset<bytes;){assert.ok(performance.now()-started<60_000);const result=await handle.write(block,0,Math.min(block.length,bytes-offset),null);assert.ok(result.bytesWritten>0);offset+=result.bytesWritten}await handle.sync()}
  finally {await handle.close()}
  return boundedFileDigest([local.files],raw)
}
export async function cloudHashManifest(plan,env,base) {
  const cloud=plan.targets.find(t=>t.kind==='cloud');ownedPath([cloud.files],base);assert.ok(base!==cloud.files)
  // One bounded read-only SHA-256 stream; no account inventory or automatic replay.
  try {
    const reply=await exec('/usr/bin/rclone',['--config',env.RCLONE_CONFIG,'--retries','1','--low-level-retries','1','--timeout','60s','--contimeout','15s','hashsum','SHA256',rclonePath(base),'--download'],{env,cwd:plan.targets[0].files,timeout:600_000,maxBuffer:512*1024})
    const hashes=new Map()
    for(const line of reply.stdout.trim().split('\n')){const match=line.match(/^([a-f0-9]{64}) {2}(.+)$/);assert.ok(match&&hashes.size<2048&&!hashes.has(match[2]));assert.ok(match[2].split('/').every(p=>p&&p!=='.'&&p!=='..'));hashes.set(match[2],match[1])}
    return hashes
  } catch(error) {if(error.name==='AssertionError')throw error;throw Object.assign(new Error('Bounded OneDrive SHA-256 verification failed; no replay'),{failureKind:'FIXTURE_IO'})}
}
async function localManifest(plan,base,expected) {
  const local=plan.targets[0],actual=new Map(),stack=[base];let entries=0
  while(stack.length){const dir=stack.pop();ownedPath([local.files],dir);await noLinks(dir,fs);const names=await fs.readdir(dir);assert.ok(names.length<=256)
    for(const name of names){assert.ok(++entries<=2048);const raw=child(dir,name),rel=raw.slice(base.length+1);await noLinks(raw,fs);const meta=await fs.lstat(raw)
      if(meta.isDirectory()){actual.set(rel,null);stack.push(raw)}else actual.set(rel,await boundedFileDigest([local.files],raw))}}
  assert.deepEqual([...actual].sort(),[...expected].sort());return {entries,files:[...actual.values()].filter(Boolean).length,bytes:[...actual.values()].reduce((sum,item)=>sum+(item?.bytes??0),0)}
}
export async function cloudScale(plan,fixture,ui,record) {
  cloudScaleManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=900_000
  const local=plan.targets[0],cloud=plan.targets[1],source=child(local.files,'cloud-scale-source'),uploaded=child(cloud.files,'cloud-scale-source'),download=child(local.files,'cloud-scale-download'),downloaded=child(download,'cloud-scale-source')
  const expected=new Map()
  await record('Bounded OneDrive mixed-size tree',async result=>{
    result.phase='setup';await fixture.mkdir(source);await fixture.mkdir(download)
    await recordPart(result,'generated-tree',async part=>{
      for(let group=0;group<8;group++){const groupName=`group-${group}`,dir=child(source,groupName);await fixture.mkdir(dir);expected.set(groupName,null);await fixture.mkdir(child(dir,'empty'));expected.set(groupName+'/empty',null)
        for(let index=0;index<128;index++){const name=`file-${String(index).padStart(3,'0')}.bin`,bytes=Buffer.alloc([0,1,64,1024][index%4],(group*17+index)%256);await fixture.write(child(dir,name),bytes);expected.set(groupName+'/'+name,{bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')})}}
      for(const [name,bytes]of [['128k',128*1024],['512k',512*1024],['2m',2*1024*1024],['8m',8*1024*1024]])expected.set(`large-${name}.bin`,await largeOwnedFixture(plan,child(source,`large-${name}.bin`),bytes))
      part.tree=await localManifest(plan,source,expected);assert.ok(part.tree.bytes<=byteLimit);part.ui='NOT_SENT';part.verification='OWNED_STREAMED_SHA256'
    })
    await recordPart(result,'upload',async part=>{result.phase='ui';part.ui='STARTED';const started=performance.now();await ui.transfer(source,cloud.files);part.timing=measuredSamples([performance.now()-started],'One generated 1028-file/eight-group tree upload; complete guarded UI workflow including navigation/clipboard/refresh; productive transfers use an inactivity limit');part.ui='ACKNOWLEDGED';await releasedResources(ui,plan.runId)})
    await recordPart(result,'remote-digests',async part=>{result.phase='verification';const actual=await cloudHashManifest(plan,fixture.env,uploaded),wanted=new Map([...expected].filter(([,value])=>value).map(([name,value])=>[name,value.sha256]));assert.deepEqual([...actual].sort(),[...wanted].sort());part.files=actual.size;part.ui='NOT_SENT';part.verification='INDEPENDENT_REMOTE_DOWNLOADED_SHA256'})
    await recordPart(result,'download',async part=>{result.phase='ui';part.ui='STARTED';const started=performance.now();await ui.transfer(uploaded,download);part.timing=measuredSamples([performance.now()-started],'Same bounded OneDrive mixed-size tree download; complete guarded UI workflow; productive transfers use an inactivity limit');part.ui='ACKNOWLEDGED';await releasedResources(ui,plan.runId)})
    await recordPart(result,'preserved-trees',async part=>{result.phase='verification';part.source=await localManifest(plan,source,expected);part.download=await localManifest(plan,downloaded,expected);part.ui='NOT_SENT';part.verification='EXACT_LOCAL_TREES_EMPTY_DIRS_SIZES_SHA256'})
  },{id:'provider-cloud-scale',providers:['local','cloud']})
}

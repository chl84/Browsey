import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {spawn} from 'node:child_process'
import {clearTimeout} from 'node:timers'
import {deflateSync} from 'node:zlib'
import {performance} from 'node:perf_hooks'
import {Key} from 'webdriverio'
import {child,ownedPath} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyByteTree} from './byte-tree.mjs'
import {startThumbnailObservation,finishThumbnailObservation,waitThumbnails,assertThumbnailSnapshots} from './mobile-thumbnails.mjs'
import {measuredSamples} from './metrics.mjs'
import {releasedResources} from './resources.mjs'

export function generatedStorageImage(seed) {
  assert.ok(Number.isSafeInteger(seed)&&seed>=0&&seed<60)
  const width=1280,height=720,rows=Buffer.alloc((width*3+1)*height)
  for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
    const index=y*(width*3+1)+1+x*3
    rows[index]=(Math.floor(x/128)*19+seed*3)%256
    rows[index+1]=(Math.floor(y/128)*23+seed*7)%256
    rows[index+2]=(Math.floor(x/128)*31+seed*11)%256
  }
  const crc=bytes=>{let value=0xffffffff;for(const byte of bytes){value^=byte;for(let i=0;i<8;i++)value=(value>>>1)^((value&1)?0xedb88320:0)}return (value^0xffffffff)>>>0}
  const chunk=(type,bytes)=>{const data=Buffer.concat([Buffer.from(type),bytes]),head=Buffer.alloc(4),tail=Buffer.alloc(4);head.writeUInt32BE(bytes.length);tail.writeUInt32BE(crc(data));return Buffer.concat([head,data,tail])}
  const header=Buffer.alloc(13);header.writeUInt32BE(width);header.writeUInt32BE(height,4);header[8]=8;header[9]=2
  const image=Buffer.concat([Buffer.from('89504e470d0a1a0a','hex'),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))])
  assert.ok(image.length<=65536,'Keep ordinary fixture byte cap')
  return image
}
export function storageManifest(plan) {
  assert.ok(plan.targets[0].kind==='local'&&plan.targets.every(t=>['local','usb','mobile','cloud'].includes(t.kind)))
  return plan.targets.map(t=>({id:`storage-performance-${t.kind}`,name:`Native storage timing: ${t.kind}`,providers:[t.kind],partIds:[...Array.from({length:5},(_,i)=>`sample-${i}`),'preserved-images']}))
}
export async function evictOwnedData(plan,target,paths) {
  assert.ok(['local','usb'].includes(target.kind)&&plan.targets.includes(target))
  for(const raw of paths)ownedPath([target.files],raw)
  const helper=new URL('../../tests/support/native_fixture_cache.py',import.meta.url)
  const p=spawn('/usr/bin/python3',['-B',helper.pathname],{env:{PATH:'/usr/bin:/bin',LANG:'C.UTF-8'},stdio:['pipe','pipe','pipe']})
  let stdout='',stderr='';p.stdout.on('data',b=>{stdout+=b;assert.ok(stdout.length<=65536)});p.stderr.on('data',b=>{stderr+=b;assert.ok(stderr.length<=65536)})
  const timer=setTimeout(()=>p.kill('SIGTERM'),10_000)
  try {
    p.stdin.end(JSON.stringify({root:target.files,paths}))
    const code=await new Promise((resolve,reject)=>{p.once('error',reject);p.once('exit',resolve)})
    assert.equal(code,0,'Owned cache observation failed; no global cache fallback')
    const result=JSON.parse(stdout);assert.equal(result.schema,1);assert.equal(result.files.length,paths.length)
    return result
  } finally {clearTimeout(timer)}
}
async function cloudThumbnails(ui) {
  await ui.chord('s');const modal=await ui.browser.$('.settings-modal');await modal.waitForDisplayed({timeout:5000})
  await ui.fill(await modal.$('.settings-filter'),'Cloud thumbs')
  const checkbox=await modal.$('input[type="checkbox"]');await checkbox.waitForExist({timeout:5000})
  if(!await checkbox.isSelected())await(await modal.$('label')).click()
  assert.equal(await checkbox.isSelected(),true)
  await ui.browser.keys([Key.Escape]);if(await modal.isExisting())await ui.browser.keys([Key.Escape]);await modal.waitForExist({reverse:true,timeout:5000});await ui.idle()
}
export async function storagePerformance(plan,fixture,ui,record) {
  storageManifest(plan);ui.waitTimeout=180_000;ui.transferTimeout=600_000
  if(plan.targets.some(t=>t.kind==='cloud'))await cloudThumbnails(ui)
  for(const target of plan.targets)await record(`Native storage timing: ${target.kind}`,async result=>{
    const preserved=[],listing=[],first=[],all=[];result.workload={samples:5,imagesPerSample:12,width:1280,height:720,format:'generated RGB PNG',maxImageBytes:65536}
    for(let sample=0;sample<5;sample++)await recordPart(result,`sample-${sample}`,async part=>{
      result.phase='setup';const base=child(target.files,`storage-sample-${sample}`),expected=new Map()
      await fixture.mkdir(base)
      for(let image=0;image<12;image++){const name=`image-${String(image).padStart(2,'0')}.png`,bytes=generatedStorageImage(sample*12+image);expected.set(name,bytes);await fixture.write(child(base,name),bytes)}
      const paths=[...expected.keys()].map(n=>child(base,n));preserved.push({base,expected})
      await ui.navigate(target.files);await ui.setView('list');await ui.sort('Name','asc')
      if(['local','usb'].includes(target.kind))part.cache=await evictOwnedData(plan,target,paths)
      else part.cache={verifiedDataCold:false,scope:'fresh names and private thumbnail cache; remote/device caches uncontrolled'}
      result.phase='ui';part.ui='STARTED'
      // Measure one actual Enter from the parent, excluding setup/root navigation.
      await ui.select(base);const started=performance.now();await ui.browser.keys(['Enter']);await ui.waitPath(base);await ui.listing(base,'list',paths,{fileOrder:paths})
      part.listingMs=performance.now()-started;listing.push(part.listingMs)
      await startThumbnailObservation(ui)
      let observations
      try {await ui.setView('grid');await ui.listing(base,'grid',paths,{fileOrder:paths});await waitThumbnails(ui,paths)}
      finally {observations=await finishThumbnailObservation(ui)}
      const decoded=observations.find(s=>s.images.some(i=>paths.includes(i.path)&&i.thumbnail&&i.decoded))
      assert.ok(decoded);assertThumbnailSnapshots(observations,paths,paths)
      part.firstThumbnailMs=decoded.time-observations[0].time;part.allThumbnailsMs=observations.at(-1).time-observations[0].time
      first.push(part.firstThumbnailMs);all.push(part.allThumbnailsMs)
      const local=plan.targets.find(t=>t.kind==='local'),artifact=`storage-${target.kind}-${sample}.json`
      await fs.writeFile(path.join(local.run,'artifacts',artifact),JSON.stringify(observations),{flag:'wx',mode:0o600});part.observationsArtifact=artifact
      part.ui='ACKNOWLEDGED';part.verification='ACTUAL_DECODED_NATIVE_THUMBNAILS';await releasedResources(ui,plan.runId)
    })
    result.listing=measuredSamples(listing,'Node monotonic clock: actual Enter from parent through exact list membership; driver and native guards included; directory metadata cache uncontrolled')
    result.firstThumbnail=measuredSamples(first,'Browser monotonic clock: observer before grid toggle through first decoded cache thumbnail; see individual verified host-data-page or uncontrolled remote-cache observations')
    result.allThumbnails=measuredSamples(all,'Browser monotonic clock: observer before grid toggle through all 12 decoded thumbnails')
    await recordPart(result,'preserved-images',async part=>{result.phase='verification';part.trees=[];for(const {base,expected}of preserved)part.trees.push(await verifyByteTree(fixture,base,expected));part.ui='NOT_SENT';part.verification='INDEPENDENT_BYTES_AFTER_MEASUREMENT'})
  },{id:`storage-performance-${target.kind}`,providers:[target.kind]})
}

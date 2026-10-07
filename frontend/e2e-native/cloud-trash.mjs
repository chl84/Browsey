import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyByteTree} from './byte-tree.mjs'
import {releasedResources} from './resources.mjs'

export const cloudTrashManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local','cloud']);return [{id:'provider-cloud-web-restore',name:'OneDrive native trash and manual web restore',providers:['local','cloud'],partIds:['native-upload','native-trash','manual-web-restore','preserved-bytes']}]}
export function cloudTrashLocations(plan){const name=`browsey-web-restore-${plan.runId}.txt`,from=child(plan.targets[0].files,'cloud-trash-source'),to=child(plan.targets[1].files,'cloud-trash-source');return {name,from,to,source:child(from,name),target:child(to,name)}}
export async function cloudTrash(plan,fixture,ui,record){
  cloudTrashManifest(plan);ui.waitTimeout=180_000
  await record('OneDrive native trash and manual web restore',async result=>{
    const p=cloudTrashLocations(plan),bytes=`Generated Browsey recycle-bin test ${plan.runId}\n`,expected=new Map([[p.name,Buffer.from(bytes)]])
    assert.equal((await ui.handshake(plan.runId)).desktopMode,'cloud-trash')
    result.phase='setup';await fixture.mkdir(p.from);await fixture.mkdir(p.to);await fixture.write(p.source,bytes)
    await recordPart(result,'native-upload',async part=>{result.phase='ui';part.ui='STARTED';await ui.transfer(p.source,p.to);result.phase='verification';await verifyByteTree(fixture,p.to,expected);await verifyByteTree(fixture,p.from,expected);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_UPLOADED_BYTES'})
    await recordPart(result,'native-trash',async part=>{result.phase='ui';part.ui='STARTED';await ui.navigate(p.to);await ui.select(p.target);await ui.menuAction('move-trash',p.target);await ui.idle({absentPath:p.target});result.phase='verification';await verifyByteTree(fixture,p.to,new Map());await verifyByteTree(fixture,p.from,expected);await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_CLOUD_ABSENCE_AND_ORIGINAL_BYTES'})
    await recordPart(result,'manual-web-restore',async part=>{
      result.phase='verification';part.name=p.name;part.started=new Date().toISOString();part.method='USER_WEB_RECYCLE_BIN_RESTORE_REQUIRED';console.log(`WEB_RESTORE_READY ${p.name}`)
      try {await ui.browser.waitUntil(()=>fixture.exists(p.target),{timeout:600_000,interval:3000,timeoutMsg:'Exact generated OneDrive file was not restored within the bounded manual web window'})}
      catch(error){throw Object.assign(error,{failureKind:'MANUAL_RESTORE_NOT_OBSERVED'})}
      await verifyByteTree(fixture,p.to,expected);part.completed=new Date().toISOString();part.verification='INDEPENDENT_RETURN_TO_EXACT_ORIGINAL_PATH_AND_BYTES'
    })
    await recordPart(result,'preserved-bytes',async part=>{result.phase='verification';await verifyByteTree(fixture,p.from,expected);await verifyByteTree(fixture,p.to,expected);await ui.navigate(p.to);await ui.idle({resultPath:p.target});await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_LOCAL_ORIGINAL_AND_RESTORED_CLOUD_BYTES'})
  },{id:'provider-cloud-web-restore',providers:['local','cloud']})
}

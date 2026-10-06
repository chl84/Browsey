import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {NativeUi} from './ui.mjs'
import {fixtureLauncher} from './desktop.mjs'
import {trackChild,checkPorts,waitDriver,ownedNativeDriverPid,captureCandidate,assertCandidateAlive,teardown,stopDriver} from './lifecycle.mjs'
const exec=promisify(execFile)

export function desktopApplications({repo,plan,local,env,candidate,tools,report,persist}) {
  let peerUsed=false,nautilusUsed=false
  return {
    async withPeer(action) {
      assert.equal(peerUsed,false,'Only one explicitly owned peer session');peerUsed=true
      const evidence={role:'browsey-peer',status:'RUNNING'};(report.desktop.applications??=[]).push(evidence);await persist()
      const log=await fs.open(path.join(local.run,'artifacts/peer-driver.log'),'wx',0o600)
      let driver,browser,nativeDriver,owner
      try {
        await checkPorts([4446,4447])
        driver=spawn(tools.driver,['--port','4446','--native-host','127.0.0.1','--native-port','4447','--native-driver',tools.webkit],{env,cwd:local.files,stdio:['ignore',log.fd,log.fd]});trackChild(driver)
        await waitDriver(driver,{probe:signal=>fetch('http://127.0.0.1:4446/status',{signal}),listening:async()=>{
          const pid=await ownedNativeDriverPid(driver,await fs.realpath(tools.webkit),4446,4447)
          if(!pid)return false
          nativeDriver=await captureCandidate(pid,{executable:await fs.realpath(tools.webkit),dataHome:env.XDG_DATA_HOME,runId:plan.runId,role:'native-driver'});return true
        }})
        const {remote}=await import('webdriverio')
        browser=await remote({hostname:'127.0.0.1',port:4446,logLevel:'silent',connectionRetryCount:0,connectionRetryTimeout:60_000,capabilities:{'tauri:options':fixtureLauncher(repo,local.run,candidate)}})
        const peer=new NativeUi(browser,plan.targets.map(t=>t.files));peer.desktop={repo,plan,local,env,candidate}
        const status=await peer.handshake(plan.runId)
        owner=await captureCandidate(status.pid,{executable:candidate,dataHome:env.XDG_DATA_HOME,runId:plan.runId});evidence.identity=owner
        await assertCandidateAlive(owner);await action(peer,evidence);await assertCandidateAlive(owner)
        evidence.status='PASS'
      } finally {
        evidence.teardown=await teardown({browser,driver,nativeDriver,candidate:owner});await log.close();await persist()
        assert.equal(evidence.teardown.status,'PASS','Owned peer teardown must be proven')
      }
    },
    async withNautilus(destination,action) {
      assert.equal(nautilusUsed,false,'Only one approved Nautilus fixture window');nautilusUsed=true
      const evidence={role:'nautilus-fixture',status:'RUNNING',backend:'x11',receiverPolicy:'external-process-copy-only'};(report.desktop.applications??=[]).push(evidence);await persist()
      const reply=await exec('/usr/bin/gdbus',['call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.NameHasOwner','org.gnome.Nautilus'],{env,timeout:3000})
      assert.equal(reply.stdout.trim(),'(false,)','Never attach to an existing Nautilus')
      const launcher=fixtureLauncher(repo,local.run,'/usr/bin/nautilus',['--new-window',destination]),log=await fs.open(path.join(local.run,'artifacts/nautilus.log'),'wx',0o600)
      const process=spawn(launcher.application,launcher.args,{env:{...env,GTK_A11Y:'atspi',GSK_RENDERER:'cairo'},cwd:local.files,stdio:['ignore',log.fd,log.fd]});trackChild(process)
      let owner
      try {
        // Inspect only this exact spawned process and its direct namespace child.
        const executable=await fs.realpath('/usr/bin/nautilus'),deadline=Date.now()+15_000
        while(Date.now()<deadline&&!owner) {
          const children=(await fs.readFile(`/proc/${process.pid}/task/${process.pid}/children`,'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)
          assert.ok(children.length<=4,'Unexpected fixture process tree')
          for(const pid of [process.pid,...children])try {
            if(await fs.readlink(`/proc/${pid}/exe`)!==executable)continue
            owner=await captureCandidate(pid,{executable,dataHome:env.XDG_DATA_HOME,runId:plan.runId,role:'nautilus-fixture'});break
          } catch(error) {if(!['ENOENT','ESRCH'].includes(error.code))throw error}
          if(!owner)await new Promise(r=>setTimeout(r,100))
        }
        assert.ok(owner,'Nautilus identity must be captured before control');evidence.identity=owner
        await assertCandidateAlive(owner);await action(owner,evidence);await assertCandidateAlive(owner);evidence.status='PASS'
      } finally {
        evidence.teardown=await teardown({candidate:owner});await stopDriver(process);await log.close();await persist()
        assert.equal(evidence.teardown.status,'PASS','Owned Nautilus teardown must be proven')
      }
    },
  }
}

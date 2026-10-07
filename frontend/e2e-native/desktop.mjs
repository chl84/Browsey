import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {randomBytes} from 'node:crypto'
import {privateStat,writePrivate,processStamp} from './privacy.mjs'
import {fileSha256} from './candidate.mjs'
import {trackChild,stopDriver} from './lifecycle.mjs'
const exec=promisify(execFile)

export async function startDesktop(repo,plan,env) {
  const origin=JSON.parse(process.env.BROWSEY_NATIVE_ISOLATED??'null')
  assert.equal(origin?.runId,plan.runId,'Desktop runner requires this wrapper run identity')
  assert.notEqual(await fs.readlink('/proc/self/ns/mnt'),origin.mount,'Private mount namespace required')
  assert.notEqual(await fs.readlink('/proc/self/ns/pid'),origin.pid,'Private PID namespace required')
  await assert.rejects(fs.access(path.join(origin.home,'.config')),{code:'ENOENT'},'Personal desktop config must not be mounted')
  const local=plan.targets.find(t=>t.kind==='local'),auth=path.join(local.run,'r/Xauthority')
  await exec('/usr/bin/xauth',['-f',auth,'add',':91','MIT-MAGIC-COOKIE-1',randomBytes(16).toString('hex')],{env})
  privateStat(await fs.stat(auth))
  const fontConfig=path.join(local.run,'r/fonts.conf')
  await writePrivate(fontConfig,'<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><dir>/usr/share/fonts</dir><cachedir>/var/cache/fontconfig</cachedir></fontconfig>\n')
  const services=[],logs=[],evidence={schema:1,runId:plan.runId,originNamespacesDifferent:true,personalConfigAbsent:true,display:':91',services:[]}
  async function launch(binary,args,serviceEnv,name) {
    const log=await fs.open(path.join(local.run,'artifacts',name+'.log'),'wx',0o600);logs.push(log)
    const child=spawn(binary,args,{env:serviceEnv,stdio:['ignore',log.fd,log.fd]});trackChild(child);services.push(child)
    evidence.services.push({name,pid:child.pid,start:(await processStamp(child.pid)).start})
    return child
  }
  const displayEnv={...env,PATH:path.join(repo,'target/native-tools/bin')+':/usr/bin:/bin',DISPLAY:':91',XAUTHORITY:auth,FONTCONFIG_FILE:fontConfig,ATSPI_DBUS_IMPLEMENTATION:'dbus-daemon',GDK_BACKEND:'x11',WEBKIT_DISABLE_DMABUF_RENDERER:'1',XDG_SESSION_TYPE:'x11',BROWSEY_NATIVE_ISOLATED:process.env.BROWSEY_NATIVE_ISOLATED}
  delete displayEnv.WAYLAND_DISPLAY
  try {
    await launch(path.join(repo,'target/native-tools/bin/Xvfb'),[':91','-screen','0','2000x1100x24','-auth',auth,'-nolisten','tcp','-noreset'],displayEnv,'xvfb')
    let ready=false
    for(let i=0;i<50;i++) {
      try {await exec('/usr/bin/xprop',['-root','_NET_SUPPORTING_WM_CHECK'],{env:displayEnv,timeout:1000});ready=true;break} catch {await new Promise(r=>setTimeout(r,100))}
    }
    assert.ok(ready,'Owned virtual display did not start')
    const layout=process.env.BROWSEY_NATIVE_INPUT_LAYOUT
    assert.ok(['us','no'].includes(layout),'Explicit private keyboard layout required')
    await exec('/usr/bin/setxkbmap',['-display',':91','-layout',layout],{env:displayEnv,timeout:2000})
    evidence.keyboard=(await exec('/usr/bin/setxkbmap',['-display',':91','-query'],{env:displayEnv,timeout:2000})).stdout.trim()
    assert.match(evidence.keyboard,new RegExp('layout:\\s+'+layout+'(?:\\s|$)'))
    const wrong=path.join(local.run,'r/wrong-authority');await writePrivate(wrong,'')
    await assert.rejects(exec('/usr/bin/xprop',['-root'],{env:{...displayEnv,XAUTHORITY:wrong},timeout:2000}),'A client without the private cookie must be denied')
    const runtime='/tmp/browsey-native-'+plan.runId;await fs.mkdir(runtime,{mode:0o700})
    const bus=`unix:path=${runtime}/bus`
    await launch('/usr/bin/dbus-daemon',['--session','--nofork',`--address=${bus}`],{...displayEnv,XDG_RUNTIME_DIR:runtime},'dbus')
    let busId
    for(let i=0;i<50;i++) {
      try {busId=(await exec('/usr/bin/gdbus',['call','--address',bus,'--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.GetId'],{env:displayEnv,timeout:1000})).stdout;break} catch {await new Promise(r=>setTimeout(r,100))}
    }
    assert.match(busId??'',/[a-f\d]{32}/)
    Object.assign(displayEnv,{DBUS_SESSION_BUS_ADDRESS:bus,XDG_DATA_DIRS:env.XDG_DATA_HOME+':/usr/share',GSETTINGS_BACKEND:'memory'})
    evidence.cookieRequired=true;evidence.privateBus=true
    evidence.tools={xvfbSha256:await fileSha256(path.join(repo,'target/native-tools/bin/Xvfb')),bwrapSha256:await fileSha256('/usr/bin/bwrap')}
    if(JSON.parse(env.BROWSEY_NATIVE_TEST_SESSION).desktop==='desktop-services')evidence.tools.xclipSha256=await fileSha256(path.join(repo,'target/native-tools/bin/xclip'))
    await writePrivate(path.join(local.run,'artifacts/isolation.json'),JSON.stringify(evidence,null,2))
    return {env:displayEnv,evidence,close:async()=>{for(const service of [...services].reverse()) await stopDriver(service);for(const log of logs)await log.close()}}
  } catch(error) {for(const service of [...services].reverse())await stopDriver(service);for(const log of logs)await log.close();throw error}
}

// Each application sees this run only, plus read-only code/system libraries.
export function fixtureLauncher(repo,run,program,args=[]) {
  assert.match(run,/\/ai_agent_testfolder\/\.bnt-[0-9a-f]{32}$/,'Exact owned desktop run required')
  assert.ok(program===path.join(repo,'target/native-test/browsey')||program==='/usr/bin/nautilus','Only the scoped candidate or approved Nautilus may launch')
  if(program==='/usr/bin/nautilus') {
    assert.equal(args.length,2);assert.equal(args[0],'--new-window')
    assert.ok(args[1].startsWith(run+'/files/')&&!args[1].split('/').some(p=>p==='..'||p==='.'),'Nautilus requires an exact generated folder')
  } else assert.deepEqual(args,[],'Candidate takes no arbitrary launch arguments')
  return {application:'/usr/bin/bwrap',args:['--die-with-parent','--unshare-user','--unshare-ipc','--unshare-uts',
    '--ro-bind','/usr','/usr','--symlink','usr/bin','/bin','--symlink','usr/lib','/lib','--symlink','usr/lib','/lib64',
    '--ro-bind','/etc/passwd','/etc/passwd','--ro-bind','/etc/group','/etc/group','--ro-bind','/etc/machine-id','/etc/machine-id','--ro-bind','/etc/fonts','/etc/fonts','--ro-bind','/var/cache/fontconfig','/var/cache/fontconfig','--ro-bind','/etc/ld.so.cache','/etc/ld.so.cache','--ro-bind','/etc/ssl','/etc/ssl','--ro-bind','/etc/ca-certificates','/etc/ca-certificates','--ro-bind','/etc/resolv.conf','/etc/resolv.conf',
    '--proc','/proc','--dev','/dev','--bind','/tmp','/tmp','--ro-bind',repo,repo,'--bind',run,run,
    '--tmpfs','/usr/share/applications','--tmpfs','/usr/local','--dir','/usr/local/share','--dir','/usr/local/share/applications','--chdir',path.join(run,'files'),'--',program,...args]}
}

// Dedicated desktop/PID/mount isolation. No shared desktop sockets or personal HOME.
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
import {randomUUID} from 'node:crypto'
import {spawn} from 'node:child_process'
import {validateConfig} from './scope.mjs'
import {privateJson,processStamp,processGone} from './privacy.mjs'

export const desktopSuites=['smoke','repeatability','measurements','cancellation','drag','drag-feedback','desktop-services','keyboard','appearance','watchers','archives','open-with','cloud-export','cloud-trash']
export function desktopArguments(args) {
  assert.ok(args.length>=2&&args[0]==='--suite'&&desktopSuites.includes(args[1]),'Explicit supported desktop suite required')
  const providers=args.includes('--targets')?args[args.indexOf('--targets')+1]:'local'
  assert.ok(['local','local,cloud'].includes(providers),'Desktop isolation permits local and explicit cloud only')
  assert.ok(providers==='local'||['archives','cloud-export','cloud-trash'].includes(args[1]),'Only explicit archive/export/trash suites may include cloud')
  for(let i=2;i<args.length;i++) {
    if(args[i]==='--targets') {i++;continue}
    assert.equal(args[i],'--a11y','Unknown desktop argument')
  }
  return {suite:args[1],providers}
}
const repo=fileURLToPath(new URL('../..',import.meta.url))
export function sandboxArgs(repo,local,network=false) {
  assert.ok(path.isAbsolute(repo)&&path.isAbsolute(local))
  return ['--die-with-parent','--unshare-user','--unshare-pid','--unshare-ipc','--unshare-uts',...(network?[]:['--unshare-net']),
    '--ro-bind','/usr','/usr','--ro-bind',process.execPath,'/native-node','--symlink','usr/bin','/bin','--symlink','usr/lib','/lib','--symlink','usr/lib','/lib64',
    '--ro-bind','/etc/fonts','/etc/fonts','--ro-bind','/var/cache/fontconfig','/var/cache/fontconfig','--ro-bind','/etc/ld.so.cache','/etc/ld.so.cache','--ro-bind','/etc/ssl','/etc/ssl','--ro-bind','/etc/ca-certificates','/etc/ca-certificates',
    '--ro-bind','/etc/resolv.conf','/etc/resolv.conf','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--dir','/run',
    '--ro-bind',repo,repo,'--bind',path.join(repo,'target/native-test/.retention'),path.join(repo,'target/native-test/.retention'),
    '--bind',local,local,'--chdir',repo]
}
async function main() {
  process.umask(0o077)
  const args=process.argv.slice(2),options=desktopArguments(args),config=validateConfig(await privateJson(path.join(repo,'frontend/e2e-native/config.local.json')))
  const local=config.targets.find(t=>t.kind==='local').path,runId=randomUUID()
  const origin={mount:await fs.readlink('/proc/self/ns/mnt'),pid:await fs.readlink('/proc/self/ns/pid'),home:process.env.HOME,runId}
  const env={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',HOME:'/tmp/native-home',BROWSEY_NATIVE_INPUT_LAYOUT:['smoke','repeatability'].includes(options.suite)?'no':'us',BROWSEY_NATIVE_ISOLATED:JSON.stringify(origin)}
  // No DISPLAY, Wayland, D-Bus, Xauthority, SSH or credential inheritance.
  const command=[...sandboxArgs(repo,local,options.providers.includes('cloud')),'--','/native-node',path.join(repo,'frontend/e2e-native/desktop-bootstrap.mjs'),'--run','--suite',options.suite,'--targets',options.providers,'--run-id',runId,...(args.includes('--a11y')?['--a11y']:[])]
  const child=spawn('/usr/bin/bwrap',command,{env,stdio:'inherit'})
  const stamp=await processStamp(child.pid)
  const code=await new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',(code,signal)=>resolve(code??(signal?1:0)))})
  assert.ok(await processGone(child.pid,stamp.start),'Sandbox supervisor exit must be confirmed')
  console.log(`Isolated desktop supervisor exited; namespace children terminated; run ${runId}`)
  process.exitCode=code
}
if(process.argv[1]===fileURLToPath(import.meta.url)) main().catch(e=>{console.error(`Isolated native run stopped: ${e.message}`);process.exitCode=1})

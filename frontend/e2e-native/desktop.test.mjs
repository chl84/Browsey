import assert from 'node:assert/strict'
import {test} from 'node:test'
import {desktopArguments,sandboxArgs} from './isolated.mjs'
import {fixtureLauncher} from './desktop.mjs'

test('desktop entry rejects other providers and arbitrary launch arguments before a process or fixture exists',()=>{
  assert.deepEqual(desktopArguments(['--suite','drag','--targets','local','--a11y']),{suite:'drag',providers:'local'})
  assert.deepEqual(desktopArguments(['--suite','archives','--targets','local,cloud']),{suite:'archives',providers:'local,cloud'})
  for(const args of [[],['--suite','foundation'],['--suite','drag','--targets','local,usb'],['--suite','drag','--targets','local,cloud'],['--suite','open-with','--exec','/outside/handler']])assert.throws(()=>desktopArguments(args))
})
test('desktop mount plan excludes shared display, bus and personal home; app launcher narrows writes to this run',()=>{
  const repo='/work/browsey',local='/owned/ai_agent_testfolder',run=local+'/.bnt-00000000000040008000000000000000'
  const whole=sandboxArgs(repo,local)
  assert.ok(whole.includes('--unshare-pid')&&whole.includes('--unshare-net')&&whole.includes('--unshare-user'))
  assert.ok(!whole.includes('/home')&&!whole.includes('/run/user')&&!whole.includes('/tmp/.X11-unix'))
  assert.ok(whole.includes('/etc/ca-certificates'),'HTTPS trust bundle symlink referents must be visible read-only')
  const launcher=fixtureLauncher(repo,run,repo+'/target/native-test/browsey')
  const binds=launcher.args.flatMap((arg,i)=>arg==='--bind'?[launcher.args.slice(i+1,i+3)]:[])
  assert.deepEqual(binds,[['/tmp','/tmp'],[run,run]])
  assert.ok(!launcher.args.includes(local),'The app must not see other registered runs')
  assert.ok(launcher.args.includes('--tmpfs')&&launcher.args.includes('/usr/share/applications'))
})
test('fixture programs reject shared roots, arbitrary executables and unscoped Nautilus locations before launch',()=>{
  const repo='/work/browsey',run='/owned/ai_agent_testfolder/.bnt-00000000000040008000000000000000'
  for(const [root,program,args] of [[run,'/bin/sh',[]],[run,'/usr/bin/nautilus',['--new-window','/home/personal']],[run,'/usr/bin/nautilus',['--new-window',run+'/files/../profile']],[run,repo+'/target/native-test/browsey',['--anything']],['/owned/ai_agent_testfolder',repo+'/target/native-test/browsey',[]]])assert.throws(()=>fixtureLauncher(repo,root,program,args))
  assert.equal(fixtureLauncher(repo,run,'/usr/bin/nautilus',['--new-window',run+'/files/receiver']).application,'/usr/bin/bwrap')
})

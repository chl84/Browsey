// Namespace-local system identity only. Never replace a host passwd/machine-id.
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import {randomUUID} from 'node:crypto'
const origin=JSON.parse(process.env.BROWSEY_NATIVE_ISOLATED??'null')
assert.ok(origin?.mount&&origin?.pid&&origin?.runId)
assert.notEqual(await fs.readlink('/proc/self/ns/mnt'),origin.mount)
assert.notEqual(await fs.readlink('/proc/self/ns/pid'),origin.pid)
assert.ok(process.getuid()>0&&process.getuid()===(await fs.stat('/etc')).uid,'Synthetic /etc must belong to this unprivileged namespace')
await fs.writeFile('/etc/passwd',`native:x:${process.getuid()}:${process.getgid()}:Native fixture:/tmp/native-home:/bin/sh\n`,{flag:'wx',mode:0o600})
await fs.writeFile('/etc/group',`native:x:${process.getgid()}:\n`,{flag:'wx',mode:0o600})
await fs.writeFile('/etc/machine-id',randomUUID().replaceAll('-','')+'\n',{flag:'wx',mode:0o600})
await fs.mkdir('/tmp/native-home',{mode:0o700})
await fs.mkdir('/tmp/.X11-unix',{mode:0o1777})
await import('./run.mjs')

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { candidateEnvironment, child, inside, makePlan, noLinks, ownedPath, validateConfig, validatePath } from './scope.mjs'

const local = '/test/ai_agent_testfolder'
const cloud = 'rclone://Test/ai_agent_testfolder'
const config = targets => validateConfig({ schema: 1, targets, rcloneConfig: `${local}/rclone.conf` })

test('example config leaves room for isolated native Unix sockets', () => {
  const example = JSON.parse(readFileSync(new URL('./config.example.json', import.meta.url), 'utf8'))
  assert.equal(makePlan(validateConfig(example), '00000000-0000-4000-8000-000000000000').targets.length, 1)
})

test('exact existing-folder spelling is mandatory, not a substring or earlier approved name', () => {
  for (const bad of ['/approved/agent_test_folder', '/approved/ai_agent_test_folder', `${local}/child`, `${local}-other`]) {
    assert.throws(() => config({ local: bad }))
  }
  assert.equal(config({ local }).targets[0].path, local)
  assert.throws(() => config({ cloud }))
  assert.throws(() => config({ local, usb: local }))
})

test('lexical scope rejects traversal, aliases, prefix siblings, implicit home and wrong schemes', () => {
  for (const bad of ['~', 'relative', 'file:///approved', 'sftp://server/folder', `${local}/../private`, `${local}//file`, `${local}/./file`, `${local}/x\\y`, `${local}/x\0`]) {
    assert.throws(() => validatePath(bad))
  }
  assert.throws(() => ownedPath([local], `${local}-sibling/file`))
  assert.equal(ownedPath([cloud], `${cloud}/file.txt`), `${cloud}/file.txt`)
  assert.ok(!inside(cloud, 'rclone://Other/ai_agent_testfolder/file.txt'))
  for (const name of ['../outside', '.', '..', '/absolute', 'nested/file']) assert.throws(() => child(local, name))
})

test('cloud credentials must be explicitly supplied within the approved local folder', () => {
  assert.equal(config({ local, cloud }).rcloneConfig, `${local}/rclone.conf`)
  for (const bad of ['/personal/rclone.conf', local, undefined]) {
    assert.throws(() => validateConfig({ schema: 1, targets: { local, cloud }, rcloneConfig: bad }))
  }
})

test('all five providers have bounded owned runs and both transfer directions', () => {
  const approved = config({ local, usb: '/usb/ai_agent_testfolder', network: '/network/ai_agent_testfolder', cloud, mobile: '/mtp/ai_agent_testfolder' })
  const id = '00000000-0000-4000-8000-000000000000'
  const hub = makePlan(approved, id)
  assert.equal(hub.routes.length, 8)
  assert.equal(hub.notConfigured.length, 0)
  hub.targets.forEach(target => assert.equal(target.files, `${target.path}/.bnt-${id.replaceAll('-', '')}/files`))
  assert.equal(makePlan({ ...approved, matrix: 'all-pairs' }, id).routes.length, 20)
  assert.deepEqual(makePlan(config({ local }), id).notConfigured, ['usb', 'network', 'cloud', 'mobile'])
  assert.throws(() => makePlan(approved, '../../outside'))
  assert.throws(() => makePlan(config({ local: `/very-long-${'x'.repeat(120)}/ai_agent_testfolder` }), id))
})

test('no-links preflight uses metadata only and stops before a linked destination', async () => {
  const calls = []
  const fs = { lstat: async value => {
    calls.push(value)
    return { isSymbolicLink: () => value === '/approved/link' }
  } }
  await assert.rejects(noLinks('/approved/link/personal/file', fs))
  assert.deepEqual(calls, ['/approved', '/approved/link'])
})

test('missing creation targets do not trigger parent listings or outside probes', async () => {
  const calls = []
  await noLinks(`${local}/new/child`, { lstat: async value => {
    calls.push(value)
    if (value.endsWith('/new')) throw Object.assign(new Error('missing'), { code: 'ENOENT' })
    return { isSymbolicLink: () => false }
  } })
  assert.equal(calls.at(-1), `${local}/new`)
})

test('candidate profile does not inherit personal credential/config variables', () => {
  const profile = `${local}/.browsey-native-owned/profile`
  const env = candidateEnvironment({ LANG: 'C.UTF-8', XDG_RUNTIME_DIR: '/run/user/test', WAYLAND_DISPLAY: 'wayland-test',
    HOME: '/personal', RCLONE_CONFIG: '/personal/secrets', RCLONE_CONFIG_PASS: 'secret',
    SSH_AUTH_SOCK: '/private/socket', AWS_SECRET_ACCESS_KEY: 'secret', NODE_OPTIONS: '--import private' }, profile, {})
  assert.equal(env.WAYLAND_DISPLAY, '/run/user/test/wayland-test')
  assert.equal(env.RCLONE_CONFIG, `${profile}/config/rclone.conf`)
  assert.equal(env.HOME, `${profile}/home`)
  for (const key of ['SSH_AUTH_SOCK', 'AWS_SECRET_ACCESS_KEY', 'RCLONE_CONFIG_PASS', 'NODE_OPTIONS']) assert.equal(env[key], undefined)
})

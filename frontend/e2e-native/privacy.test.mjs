import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { fileURLToPath } from 'node:url'
import { createServer } from 'node:net'
import { randomUUID } from 'node:crypto'
import { validateConfig, makePlan } from './scope.mjs'
import { createLocalSession } from './fixtures.mjs'
import { privateJson, writePrivate, privateStat, inspectTree, retentionPolicy } from './privacy.mjs'

const exec = promisify(execFile)
const repo = fileURLToPath(new URL('../..', import.meta.url))
async function directory(t) {
  const raw = await fs.mkdtemp('/tmp/bnp-')
  t.after(() => fs.rm(raw, { recursive: true }))
  return raw
}

test('private metadata requires mode 600, matching user and a single regular-file link before parsing', async t => {
  const root = await directory(t)
  const raw = `${root}/metadata.json`
  await writePrivate(raw, '{"generated":true}', { exclusive: true })
  assert.deepEqual(await privateJson(raw), { generated: true })
  await fs.chmod(raw, 0o644)
  await assert.rejects(privateJson(raw), /mode 700\/600/)
  await fs.chmod(raw, 0o600)
  const stat = await fs.lstat(raw)
  assert.throws(() => privateStat({ ...stat, uid: stat.uid + 1, isFile: () => true }), /current user/)
  await fs.symlink(raw, `${root}/link.json`)
  await assert.rejects(privateJson(`${root}/link.json`), /symlinks/)
  await fs.link(raw, `${root}/hard.json`)
  await assert.rejects(privateJson(raw), /hard links/)
  await assert.rejects(writePrivate(raw, 'overwritten'), /hard links/)
  assert.equal(await fs.readFile(`${root}/hard.json`, 'utf8'), '{"generated":true}')
})

test('private writes reject reused exclusive names and shorten existing owned files without leftover bytes', async t => {
  const root = await directory(t)
  const raw = `${root}/report.json`
  await writePrivate(raw, '{"value":"a longer fixture"}', { exclusive: true })
  await assert.rejects(writePrivate(raw, 'overwrite', { exclusive: true }), error => error.code === 'EEXIST')
  await writePrivate(raw, '{"value":1}')
  assert.deepEqual(await privateJson(raw), { value: 1 })
})

test('scoped umask protects implicit screenshot/profile creation in the child without a desktop change', async t => {
  const root = await directory(t)
  await exec(process.execPath, ['-e', "process.umask(0o077);const fs=require('node:fs');fs.mkdirSync(process.argv[1]+'/profile');fs.writeFileSync(process.argv[1]+'/profile/failure.png','synthetic screenshot')", root])
  assert.equal((await fs.stat(`${root}/profile`)).mode & 0o777, 0o700)
  assert.equal((await fs.stat(`${root}/profile/failure.png`)).mode & 0o777, 0o600)
  assert.equal((await inspectTree(root)).entries.length, 3)
})

test('tree audit rejects outside symlinks, hardlinks, non-private files and directories using metadata only', async t => {
  const root = await directory(t)
  await fs.mkdir(`${root}/run`, { mode: 0o700 })
  await writePrivate(`${root}/outside`, 'outside sentinel', { exclusive: true })
  await fs.symlink(`${root}/outside`, `${root}/run/link`)
  await assert.rejects(inspectTree(`${root}/run`), /symlinks/)
  await fs.unlink(`${root}/run/link`)
  await fs.link(`${root}/outside`, `${root}/run/hard`)
  await assert.rejects(inspectTree(`${root}/run`), /hard links/)
  await fs.unlink(`${root}/run/hard`)
  await fs.writeFile(`${root}/run/leaky`, 'generated', { mode: 0o644 })
  await fs.chmod(`${root}/run/leaky`, 0o644)
  await assert.rejects(inspectTree(`${root}/run`), /mode 700\/600/)
  await fs.chmod(`${root}/run/leaky`, 0o600)
  await fs.mkdir(`${root}/run/public`, { mode: 0o755 })
  await fs.chmod(`${root}/run/public`, 0o755)
  await assert.rejects(inspectTree(`${root}/run`), /mode 700\/600/)
  assert.equal(await fs.readFile(`${root}/outside`, 'utf8'), 'outside sentinel')
})

test('audit byte, entry, depth and elapsed-time budgets stop traversal without modifying data', async t => {
  const root = await directory(t)
  await fs.mkdir(`${root}/nested`, { mode: 0o700 })
  await writePrivate(`${root}/nested/data`, 'generated data', { exclusive: true })
  for (const changed of [{ maxRunBytes: 1 }, { maxEntries: 1 }, { maxDepth: 0 }]) {
    await assert.rejects(inspectTree(root, { policy: { ...retentionPolicy, ...changed } }), /budget exceeded/)
  }
  let time = 0
  await assert.rejects(inspectTree(root, { policy: { ...retentionPolicy, auditMs: 1 }, now: () => time++ }), /budget exceeded/)
  assert.equal(await fs.readFile(`${root}/nested/data`, 'utf8'), 'generated data')
})

test('AT-SPI-style sockets remain confined by private run directories without relaxing file permissions', async t => {
  const root = await directory(t)
  await fs.mkdir(`${root}/runtime`, { mode: 0o700 })
  const server = createServer()
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(`${root}/runtime/socket`, resolve) })
  t.after(() => new Promise(resolve => server.close(resolve)))
  await fs.chmod(`${root}/runtime/socket`, 0o777)
  assert.equal((await inspectTree(root)).entries.find(entry => entry.type === 'socket').mode, 0o777)
  await fs.chmod(`${root}/runtime`, 0o755)
  await assert.rejects(inspectTree(root), /mode 700\/600/)
})

test('test-only credentials copy into a private profile and invalid credentials stop before run creation', async t => {
  for (const variant of ['private', 'public', 'symlink', 'hardlink']) {
    const temporary = await fs.mkdtemp('/tmp/p')
    t.after(() => fs.rm(temporary, { recursive: true }))
    const root = `${temporary}/ai_agent_testfolder`
    await fs.mkdir(root, { mode: 0o700 })
    const credential = `${root}/rclone.conf`
    const generated = '[GeneratedTest]\ntoken = synthetic-credential\n'
    await writePrivate(credential, generated, { exclusive: true })
    if (variant === 'public') await fs.chmod(credential, 0o644)
    if (variant === 'hardlink') await fs.link(credential, `${root}/alias`)
    if (variant === 'symlink') {
      await fs.rename(credential, `${root}/original`)
      await fs.symlink(`${root}/original`, credential)
    }
    const config = validateConfig({ schema: 1, targets: { local: root, cloud: 'rclone://GeneratedTest/ai_agent_testfolder' },
      rcloneConfig: credential })
    const plan = makePlan(config, randomUUID())
    if (variant === 'private') {
      const profile = await createLocalSession(plan, config)
      const copied = `${profile}/config/rclone.conf`
      assert.equal(await fs.readFile(copied, 'utf8'), generated)
      privateStat(await fs.lstat(copied))
      privateStat(await fs.lstat(`${profile}/config`), true)
    } else {
      await assert.rejects(createLocalSession(plan, config))
      await assert.rejects(fs.lstat(plan.targets.find(target => target.kind === 'local').run), error => error.code === 'ENOENT')
    }
  }
})

test('Git excludes machine approvals, credentials, reports, screenshots, profiles and retention metadata', async () => {
  for (const raw of ['frontend/e2e-native/config.local.json', 'frontend/e2e-native/synthetic.local.json',
    'target/native-test/.retention/registry.json', 'target/native-test/report-results.json',
    'native-test-results/failure.png', 'ai_agent_testfolder/rclone.conf',
    'synthetic/.bnt-00000000000040008000000000000000/report.json',
    'synthetic/.bnt-00000000000040008000000000000000/profile/config/rclone.conf',
    'synthetic/.bnt-00000000000040008000000000000000/artifacts/failure.png']) {
    await exec('git', ['check-ignore', '--no-index', '--quiet', '--', raw], { cwd: repo })
    await assert.rejects(exec('git', ['ls-files', '--error-unmatch', '--', raw], { cwd: repo }), error => error.code === 1)
  }
  for (const raw of ['frontend/e2e-native/config.example.json', 'frontend/e2e-native/privacy.mjs']) {
    await assert.rejects(exec('git', ['check-ignore', '--no-index', '--quiet', '--', raw], { cwd: repo }), error => error.code === 1)
  }
})

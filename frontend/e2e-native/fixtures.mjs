import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { constants } from 'node:fs'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { randomUUID } from 'node:crypto'
import { child, noLinks, ownedPath, inside } from './scope.mjs'

const exec = promisify(execFile)
export const payload = 'Browsey native fixture: generated, non-personal data.\n'.repeat(64)

export async function regularFile(raw) {
  await noLinks(raw, fs)
  const handle = await fs.open(raw, constants.O_RDONLY | constants.O_NOFOLLOW)
  try {
    const stat = await handle.stat()
    assert.ok(stat.isFile() && stat.size <= 1024 * 1024, 'Expected a bounded regular file')
    return { text: await handle.readFile('utf8'), stat }
  } finally { await handle.close() }
}

export async function localEntryExists(roots, raw, filesystem = fs) {
  ownedPath(roots, raw)
  assert.ok(!raw.startsWith('rclone://'), 'Local verifier requires a filesystem path')
  await noLinks(raw, filesystem)
  // GVFS/MTP can retain an old file-id mapping at a moved path. Its current
  // parent listing is authoritative; never list above an owned data root.
  const parent = raw.slice(0, raw.lastIndexOf('/'))
  if (roots.some(root => inside(root, parent))) {
    await noLinks(parent, filesystem)
    try { return (await filesystem.readdir(parent)).includes(raw.slice(raw.lastIndexOf('/') + 1)) }
    catch (error) { if (error.code === 'ENOENT') return false; throw error }
  }
  try { await filesystem.lstat(raw); return true } catch (error) {
    if (error.code === 'ENOENT') return false
    throw error
  }
}

export function rclonePath(raw) {
  assert.ok(raw.startsWith('rclone://'))
  const tail = raw.slice(9)
  const slash = tail.indexOf('/')
  assert.ok(slash > 0)
  return `${tail.slice(0, slash)}:${tail.slice(slash + 1)}`
}

export class Fixtures {
  constructor(plan, env) {
    this.roots = plan.targets.map(target => target.files)
    this.local = plan.targets.find(target => target.kind === 'local')
    this.targets = plan.targets
    this.env = env
  }

  async #rclone(args) {
    // No inherited credentials/config overrides, shell, discovery or unbounded retries.
    try {
      const result = await exec('/usr/bin/rclone', ['--config', this.env.RCLONE_CONFIG,
        '--retries', '1', '--low-level-retries', '1', '--timeout', '20s', '--contimeout', '10s', ...args],
      { env: this.env, cwd: this.local.files, timeout: 45_000, maxBuffer: 1024 * 1024 })
      return result.stdout
    } catch (error) {
      // Keep provider stderr private: it can include paths, URLs or credentials.
      // Report only structured transport facts and the bounded operation name.
      const operation = args[0]
      const detail = error.killed ? 'timed out' : `exit ${Number.isInteger(error.code) ? error.code : 'unknown'}`
      throw Object.assign(new Error(`Scoped rclone ${operation} failed (${detail}); no automatic mutation retry`),
        { failureKind: 'FIXTURE_IO', operation })
    }
  }

  async ensureCloudRoot(target) {
    assert.ok(this.targets.includes(target), 'Only this plan can authorize cloud initialization')
    const metadata = JSON.parse(await this.#rclone(['lsjson', rclonePath(target.path), '--stat']))
    assert.ok(metadata?.IsDir, 'The approved cloud test folder must already exist')
    // Check this UUID path for collisions. Never adopt or list an existing run.
    const existing = JSON.parse(await this.#rclone(['lsjson', rclonePath(target.path)]))
    // Only the approved test directory is listed; no parent/account discovery.
    assert.ok(!existing.some(entry => entry.Name === target.run.split('/').at(-1)), 'Run collision')
    await this.#rclone(['mkdir', rclonePath(target.files)])
  }

  async mkdir(raw) {
    ownedPath(this.roots, raw)
    if (raw.startsWith('rclone://')) return this.#rclone(['mkdir', rclonePath(raw)])
    await noLinks(raw, fs)
    await fs.mkdir(raw, { mode: 0o700 })
  }

  async write(raw, content = payload) {
    ownedPath(this.roots, raw)
    assert.ok(Buffer.byteLength(content) <= 64 * 1024, 'Foundation fixtures are small')
    if (raw.startsWith('rclone://')) {
      assert.ok(!await this.exists(raw), 'Fixture creation must not overwrite')
      const staging = child(this.local.files, `upload-${randomUUID()}.txt`)
      await this.write(staging, content)
      await this.#rclone(['copyto', staging, rclonePath(raw), '--immutable'])
      // Keep the tiny staging fixture; no ambiguous cleanup/destructive retry.
      return
    }
    await noLinks(raw, fs)
    await fs.writeFile(raw, content, { flag: 'wx', mode: 0o600 })
  }

  async exists(raw) {
    ownedPath(this.roots, raw)
    if (raw.startsWith('rclone://')) {
      const parent = raw.slice(0, raw.lastIndexOf('/'))
      ownedPath(this.roots, parent)
      const items = JSON.parse(await this.#rclone(['lsjson', rclonePath(parent)]))
      return items.some(entry => entry.Name === raw.split('/').at(-1))
    }
    return localEntryExists(this.roots, raw)
  }

  async read(raw) {
    ownedPath(this.roots, raw)
    if (raw.startsWith('rclone://')) return this.#rclone(['cat', rclonePath(raw)])
    return (await regularFile(raw)).text
  }
}

export async function createLocalSession(plan, config, { step = async (_metadata, action) => action(),
  onOwned = async () => {} } = {}) {
  const local = plan.targets.find(target => target.kind === 'local')
  for (const target of plan.targets.filter(target => target.kind !== 'cloud')) {
    await step({ id: `approved-root-${target.kind}`, providers: [target.kind] }, async () => {
      await noLinks(target.path, fs)
      assert.ok((await fs.lstat(target.path)).isDirectory(), 'Approved folders must already exist')
    })
  }
  let cloudConfig = ''
  if (config.rcloneConfig) {
    await step({ id: 'private-cloud-credentials', providers: ['cloud'] }, async () => {
      assert.ok(inside(local.path, config.rcloneConfig))
      const credential = await regularFile(config.rcloneConfig)
      assert.equal(credential.stat.mode & 0o777, 0o600, 'Test credential config must have mode 600')
      const sections = [...credential.text.matchAll(/^\[([^\]\r\n]+)\]\s*$/gm)].map(match => match[1])
      const cloud = plan.targets.find(target => target.kind === 'cloud')
      assert.deepEqual(sections, [cloud.path.slice(9).split('/')[0]], 'Only the explicitly approved remote may be present')
      cloudConfig = credential.text
    })
  }
  // Exclusive creation: no recursive mkdir of approved roots, no reused run/profile.
  for (const target of plan.targets.filter(target => target.kind !== 'cloud')) {
    await step({ id: `owned-run-${target.kind}`, providers: [target.kind] }, async () => {
      await fs.mkdir(target.run, { mode: 0o700 })
      if (target.kind === 'local') {
        await fs.writeFile(child(local.run, 'owner.json'), JSON.stringify({ schema: 1, runId: plan.runId }), { flag: 'wx', mode: 0o600 })
        await onOwned()
      }
      await fs.mkdir(target.files, { mode: 0o700 })
    })
  }
  const profile = child(local.run, 'profile')
  await step({ id: 'private-profile', providers: plan.targets.map(target => target.kind) }, async () => {
    await fs.mkdir(profile, { mode: 0o700 })
    for (const leaf of ['data', 'config', 'cache', 'state', 'home']) {
      await fs.mkdir(child(profile, leaf), { mode: 0o700 })
    }
    for (const leaf of ['r', 't']) await fs.mkdir(child(local.run, leaf), { mode: 0o700 })
    await fs.writeFile(`${profile}/config/rclone.conf`, cloudConfig, { flag: 'wx', mode: 0o600 })
  })
  return profile
}

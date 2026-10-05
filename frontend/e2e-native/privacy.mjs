import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { createHash, randomUUID } from 'node:crypto'
import { child, noLinks } from './scope.mjs'

export const retentionPolicy = Object.freeze({ schema: 1, successDays: 7, maxRunBytes: 128 * 1024 * 1024,
  maxTotalBytes: 512 * 1024 * 1024, maxRuns: 40, maxEntries: 10_000, maxDepth: 32, auditMs: 10_000 })
export const rootHash = root => createHash('sha256').update(root).digest('hex')

export function privateStat(stat, directory = false) {
  assert.equal(Number(stat.uid), process.getuid(), 'Private native data must belong to the current user')
  assert.ok(directory ? stat.isDirectory() : stat.isFile(), 'Expected a private directory or regular file')
  assert.equal(Number(stat.mode) & 0o777, directory ? 0o700 : 0o600, 'Private native directories/files require mode 700/600')
  if (!directory) assert.equal(Number(stat.nlink), 1, 'Private files cannot have other hard links')
}

export async function assertPrivateFile(raw) {
  await noLinks(raw, fs)
  const handle = await fs.open(raw, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW)
  try { const stat = await handle.stat(); privateStat(stat); return stat }
  finally { await handle.close() }
}

export async function privateJson(raw) {
  await noLinks(raw, fs)
  const handle = await fs.open(raw, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW)
  try {
    const stat = await handle.stat(); privateStat(stat)
    assert.ok(stat.size <= 1024 * 1024, 'Private metadata must be bounded')
    return JSON.parse(await handle.readFile('utf8'))
  } finally { await handle.close() }
}

export async function writePrivate(raw, text, { exclusive = false } = {}) {
  await noLinks(raw, fs)
  const handle = await fs.open(raw, fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_NOFOLLOW
    | (exclusive ? fs.constants.O_EXCL : 0), 0o600)
  try {
    privateStat(await handle.stat())
    await handle.truncate(0)
    await handle.writeFile(text)
  } finally { await handle.close() }
}

export async function processStamp(pid) {
  assert.ok(Number.isSafeInteger(pid) && pid > 1, 'Expected an exact process PID')
  const raw = await fs.readFile(`/proc/${pid}/stat`, 'utf8')
  const fields = raw.slice(raw.lastIndexOf(')') + 2).split(' ')
  return { start: fields[19], state: fields[0] }
}

export async function processGone(pid, start) {
  assert.ok(typeof start === 'string' && /^\d+$/.test(start), 'Process start time must be recorded')
  try { const actual = await processStamp(pid); return actual.start !== start || ['Z', 'X'].includes(actual.state) }
  catch (error) { if (['ENOENT', 'ESRCH'].includes(error.code)) return true; throw error }
}

export async function makeOwner(plan, nonce = randomUUID()) {
  const local = plan.targets.find(target => target.kind === 'local')
  return { schema: 2, runId: plan.runId, createdAt: new Date().toISOString(), nonce,
    rootHash: rootHash(local.path), selectedProviders: plan.targets.map(target => target.kind),
    runner: { pid: process.pid, start: (await processStamp(process.pid)).start }, policy: retentionPolicy.schema }
}

function identity(stat, type) {
  return { type, device: String(stat.dev), inode: String(stat.ino), size: Number(stat.size),
    mode: Number(stat.mode) & 0o777, modified: String(stat.mtimeNs) }
}

export async function inspectTree(run, { policy = retentionPolicy, now = Date.now } = {}) {
  await noLinks(run, fs)
  const entries = []
  const deadline = now() + policy.auditMs
  let bytes = 0
  const walk = async (raw, relative, depth) => {
    assert.ok(now() <= deadline && depth <= policy.maxDepth && entries.length < policy.maxEntries,
      'Native audit depth/entry/time budget exceeded')
    await noLinks(raw, fs)
    const stat = await fs.lstat(raw, { bigint: true })
    assert.ok(!stat.isSymbolicLink(), 'Owned-run audit refuses symlinks')
    assert.equal(Number(stat.uid), process.getuid(), 'Owned-run entry has a different owner')
    const type = stat.isDirectory() ? 'directory' : stat.isFile() ? 'file' : stat.isSocket() ? 'socket' : 'unsupported'
    assert.notEqual(type, 'unsupported', 'Owned-run audit refuses devices/FIFOs and unknown entry types')
    if (type !== 'socket') privateStat(stat, type === 'directory')
    // AT-SPI creates a mode-777 Unix socket even with umask 077. It contains
    // no persisted file data; the audited mode-700 run and every parent
    // directory prevent other users from reaching it. Never relax file modes.
    if (type === 'file') bytes += Number(stat.size)
    assert.ok(bytes <= policy.maxRunBytes, 'Native per-run byte budget exceeded')
    entries.push({ path: relative, ...identity(stat, type) })
    if (type === 'directory') {
      await noLinks(raw, fs)
      const directory = await fs.opendir(raw)
      for await (const item of directory) await walk(child(raw, item.name), relative ? `${relative}/${item.name}` : item.name, depth + 1)
    }
  }
  await walk(run, '', 0)
  entries.sort((a, b) => a.path.localeCompare(b.path))
  return { bytes, entries, sha256: createHash('sha256').update(JSON.stringify(entries)).digest('hex') }
}

export async function verifyEntry(raw, expected) {
  await noLinks(raw, fs)
  const stat = await fs.lstat(raw, { bigint: true })
  const type = stat.isDirectory() ? 'directory' : stat.isFile() ? 'file' : stat.isSocket() ? 'socket' : 'unsupported'
  const actual = identity(stat, type)
  assert.equal(Number(stat.uid), process.getuid())
  for (const key of ['type', 'device', 'inode', 'mode']) assert.equal(actual[key], expected[key], 'Owned cleanup identity changed')
  if (type === 'file') {
    assert.equal(Number(stat.nlink), 1)
    assert.equal(actual.size, expected.size)
    assert.equal(actual.modified, expected.modified)
  }
}

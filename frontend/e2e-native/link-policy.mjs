import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import path from 'node:path'
import { child, noLinks } from './scope.mjs'

const identity = stat => ({ device: String(stat.dev), inode: String(stat.ino) })
const same = (stat, expected) => String(stat.dev) === expected.device && String(stat.ino) === expected.inode
const ownedStat = stat => assert.equal(Number(stat.uid), process.getuid(), 'Owned link owner changed')
export function linkPlan(plan) {
  const files = plan.targets.find(target => target.kind === 'local').files
  const source = child(files, 'links-source'), moved = child(files, 'links-moved')
  return { symlinks: [
    { path: child(source, 'relative-link'), target: child(source, 'target.txt'), linkText: 'target.txt', broken: false },
    { path: child(source, 'broken-link'), target: child(source, 'missing.txt'), linkText: 'missing.txt', broken: true },
  ], hardlinks: [{ paths: [child(source, 'hard-source.txt'), child(source, 'hard-alias.txt'), child(moved, 'hard-alias.txt')] }] }
}
export function validateLinkPolicy(run, policy) {
  assert.equal(policy.schema, 1); assert.match(policy.runId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
  assert.equal(path.basename(run), `.bnt-${policy.runId.replaceAll('-', '')}`)
  assert.ok(Array.isArray(policy.symlinks) && policy.symlinks.length <= 4)
  assert.ok(Array.isArray(policy.hardlinks) && policy.hardlinks.length <= 4)
  const files = child(run, 'files'), seen = new Set()
  const owned = raw => {
    assert.ok(typeof raw === 'string' && raw.startsWith(`${files}/`) && path.normalize(raw) === raw && !raw.includes('\0'), 'Link plan requires exact owned paths')
  }
  const unique = raw => { owned(raw); assert.ok(!seen.has(raw), 'Duplicate owned link path'); seen.add(raw) }
  const captured = item => {
    assert.ok(/^[0-9]+$/.test(item.device) && /^[0-9]+$/.test(item.inode), 'Captured link identity required')
  }
  for (const link of policy.symlinks) {
    unique(link.path); owned(link.target); captured(link)
    assert.equal(typeof link.broken, 'boolean')
    assert.ok(typeof link.linkText === 'string' && link.linkText.length > 0 && !/[/\\\0]/.test(link.linkText) && !['.', '..'].includes(link.linkText))
    assert.equal(path.join(path.dirname(link.path), link.linkText), link.target, 'Only an exact sibling relative referent is approved')
  }
  for (const group of policy.hardlinks) {
    assert.ok(Array.isArray(group.paths) && group.paths.length >= 2 && group.paths.length <= 4)
    captured(group); for (const raw of group.paths) unique(raw)
  }
  return policy
}
export async function captureLinkPolicy(run, runId, plan) {
  const symlinks = []
  for (const link of plan.symlinks) {
    await noLinks(path.dirname(link.path), fs)
    const stat = await fs.lstat(link.path, { bigint: true }); ownedStat(stat); assert.ok(stat.isSymbolicLink())
    symlinks.push({ ...link, ...identity(stat) })
  }
  const hardlinks = []
  for (const group of plan.hardlinks) {
    await noLinks(group.paths[0], fs)
    const stat = await fs.lstat(group.paths[0], { bigint: true }); ownedStat(stat); assert.ok(stat.isFile())
    hardlinks.push({ ...group, ...identity(stat) })
  }
  const policy = validateLinkPolicy(run, { schema: 1, runId, symlinks, hardlinks })
  for (const link of symlinks) await verifyOwnedSymlink(policy, link.path)
  for (const group of hardlinks) await verifyOwnedHardlink(policy, group.paths[0])
  return policy
}
export async function verifyOwnedSymlink(policy, raw, stat) {
  const link = policy?.symlinks.find(link => link.path === raw)
  assert.ok(link, 'Undeclared owned symlinks are forbidden'); await noLinks(path.dirname(raw), fs)
  stat ??= await fs.lstat(raw, { bigint: true }); ownedStat(stat)
  assert.ok(stat.isSymbolicLink() && same(stat, link) && Number(stat.nlink) === 1, 'Owned symlink identity changed')
  assert.equal(await fs.readlink(raw), link.linkText, 'Owned relative link changed')
  await noLinks(link.target, fs)
  let target
  try { target = await fs.lstat(link.target); } catch (error) { if (error.code !== 'ENOENT') throw error }
  if (link.broken) assert.equal(target, undefined, 'Broken referent unexpectedly exists')
  else { assert.ok(target?.isFile(), 'Owned referent changed'); ownedStat(target); assert.equal(target.nlink, 1) }
}
export async function verifyOwnedHardlink(policy, raw, stat) {
  const group = policy?.hardlinks.find(group => group.paths.includes(raw))
  assert.ok(group, 'Undeclared owned hard links are forbidden'); await noLinks(raw, fs)
  stat ??= await fs.lstat(raw, { bigint: true }); ownedStat(stat)
  assert.ok(stat.isFile() && same(stat, group) && Number(stat.nlink) === 2, 'Owned hard-link identity or alias count changed')
  let count = 0
  for (const alias of group.paths) {
    await noLinks(alias, fs)
    let other
    try { other = await fs.lstat(alias, { bigint: true }) } catch (error) { if (error.code !== 'ENOENT') throw error }
    if (other) { ownedStat(other); assert.ok(other.isFile() && same(other, group) && Number(other.nlink) === 2, 'Owned hard-link group changed'); count++ }
  }
  assert.equal(count, 2, 'Hard links require exactly two known aliases')
}
export async function readOwnedHardlink(policy, raw) {
  await verifyOwnedHardlink(policy, raw)
  const handle = await fs.open(raw, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW)
  try {
    const stat = await handle.stat({ bigint: true }); await verifyOwnedHardlink(policy, raw, stat)
    assert.ok(stat.size <= 65536n, 'Owned hard-link read budget exceeded')
    return await handle.readFile()
  } finally { await handle.close() }
}

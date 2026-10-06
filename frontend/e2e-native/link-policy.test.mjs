import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { makePlan, validateConfig } from './scope.mjs'
import { linkPlan, captureLinkPolicy, validateLinkPolicy, verifyOwnedSymlink, verifyOwnedHardlink, readOwnedHardlink } from './link-policy.mjs'
import { writePrivate, inspectTree } from './privacy.mjs'

async function setup(t) {
  const temp = await fs.mkdtemp('/tmp/n'); t.after(() => fs.rm(temp, { recursive: true }))
  const root = `${temp}/ai_agent_testfolder`; await fs.mkdir(root, { mode: 0o700 })
  const plan = makePlan(validateConfig({ schema: 1, targets: { local: root } }), randomUUID()), local = plan.targets[0]
  await fs.mkdir(local.run, { mode: 0o700 }); await fs.mkdir(local.files, { mode: 0o700 })
  const source = `${local.files}/links-source`, moved = `${local.files}/links-moved`
  await fs.mkdir(source, { mode: 0o700 }); await fs.mkdir(moved, { mode: 0o700 })
  await writePrivate(`${source}/target.txt`, 'owned referent', { exclusive: true })
  await writePrivate(`${source}/hard-source.txt`, Buffer.from([0, 255, 128]), { exclusive: true })
  const approved = linkPlan(plan)
  for (const link of approved.symlinks) await fs.symlink(link.linkText, link.path)
  await fs.link(`${source}/hard-source.txt`, `${source}/hard-alias.txt`)
  const policy = await captureLinkPolicy(local.run, plan.runId, approved)
  const save = () => writePrivate(`${local.run}/link-policy.json`, JSON.stringify(policy))
  await save(); return { temp, local, source, moved, policy, save }
}
test('exact captured leaf links are audited without traversing referents; known hard alias moves preserve bytes', async t => {
  const f = await setup(t)
  const tree = await inspectTree(f.local.run)
  assert.equal(tree.entries.filter(entry => entry.type === 'symlink').length, 2)
  assert.deepEqual(await readOwnedHardlink(f.policy, `${f.source}/hard-source.txt`), Buffer.from([0, 255, 128]))
  await fs.rename(`${f.source}/hard-alias.txt`, `${f.moved}/hard-alias.txt`)
  await verifyOwnedHardlink(f.policy, `${f.moved}/hard-alias.txt`)
  assert.equal((await inspectTree(f.local.run)).entries.filter(entry => entry.type === 'symlink').length, 2)
})
test('outside link plans and changed symlink identities fail before referent read', async t => {
  const f = await setup(t)
  const changed = globalThis.structuredClone(f.policy); changed.symlinks[0].target = `${f.temp}/outside`
  assert.throws(() => validateLinkPolicy(f.local.run, changed), /owned paths/)
  const link = f.policy.symlinks[0]
  await fs.unlink(link.path); await fs.symlink(`${f.temp}/outside`, link.path)
  await assert.rejects(verifyOwnedSymlink(f.policy, link.path))
  await assert.rejects(inspectTree(f.local.run))
})
test('unknown outside hard alias blocks audit and byte reads without following that alias', async t => {
  const f = await setup(t), alias = `${f.temp}/outside-alias`
  await fs.link(`${f.source}/hard-source.txt`, alias)
  await assert.rejects(inspectTree(f.local.run), /alias count/)
  await assert.rejects(readOwnedHardlink(f.policy, `${f.source}/hard-source.txt`), /alias count/)
  await fs.unlink(alias); await inspectTree(f.local.run)
})
test('absent manifest, unsafe metadata mode and parent-link replacement keep default refusal', async t => {
  const f = await setup(t), manifest = `${f.local.run}/link-policy.json`
  await fs.chmod(manifest, 0o644); await assert.rejects(inspectTree(f.local.run), /700\/600/)
  await fs.chmod(manifest, 0o600); await fs.unlink(manifest)
  await assert.rejects(inspectTree(f.local.run))
  await f.save()
  const movedSource = `${f.local.files}/renamed-source`
  await fs.rename(f.source, movedSource); await fs.symlink('renamed-source', f.source)
  await assert.rejects(inspectTree(f.local.run))
})
test('unexpected referent for a declared broken link refuses audit', async t => {
  const f = await setup(t), link = f.policy.symlinks.find(link => link.broken)
  await writePrivate(link.target, 'unexpected', { exclusive: true })
  await assert.rejects(inspectTree(f.local.run), /Broken referent/)
})

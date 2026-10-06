import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { child, noLinks } from './scope.mjs'
import { writePrivate } from './privacy.mjs'
import { linkPlan, captureLinkPolicy, verifyOwnedSymlink, verifyOwnedHardlink, readOwnedHardlink } from './link-policy.mjs'
import { sha256 } from './byte-tree.mjs'
import { waitActivityGone } from './progress.mjs'
import { transferListeners } from './cancellation.mjs'
import { recordPart } from './report.mjs'

const parts = ['list-relative-broken', 'relative-copy-rejection', 'broken-copy-rejection', 'hardlink-copy', 'hardlink-move']
export const linksManifest = () => [{ id: 'links-local', name: 'local: exact owned leaf links', providers: ['local'], partIds: parts }]
export async function links(plan, fixture, ui, record) {
  assert.deepEqual(plan.targets.map(target => target.kind), ['local'])
  const local = plan.targets[0], source = child(local.files, 'links-source'), copied = child(local.files, 'links-copied'), moved = child(local.files, 'links-moved')
  await record('local: exact owned leaf links', async result => {
    result.phase = 'setup'
    for (const raw of [source, copied, moved]) await fixture.mkdir(raw)
    const targetBytes = Buffer.from('Owned relative referent remains intact.\n'), hardBytes = Buffer.from([0, 255, 128, 1, 2, 3])
    await fixture.write(child(source, 'target.txt'), targetBytes)
    await fixture.write(child(source, 'hard-source.txt'), hardBytes)
    const approved = linkPlan(plan)
    for (const link of approved.symlinks) { await noLinks(link.path, fs); await fs.symlink(link.linkText, link.path) }
    await noLinks(child(source, 'hard-alias.txt'), fs)
    await fs.link(child(source, 'hard-source.txt'), child(source, 'hard-alias.txt'))
    const policy = await captureLinkPolicy(local.run, plan.runId, approved)
    await writePrivate(child(local.run, 'link-policy.json'), JSON.stringify(policy, null, 2), { exclusive: true })
    let didCopy = false, didMove = false
    const verify = async part => {
      for (const link of policy.symlinks) await verifyOwnedSymlink(policy, link.path)
      await verifyOwnedHardlink(policy, child(source, 'hard-source.txt'))
      assert.deepEqual(await readOwnedHardlink(policy, child(source, 'hard-source.txt')), hardBytes)
      assert.deepEqual(await readOwnedHardlink(policy, child(didMove ? moved : source, 'hard-alias.txt')), hardBytes)
      assert.deepEqual(await fixture.readBytes(child(source, 'target.txt')), targetBytes)
      for (const [base, expected] of [[source, ['target.txt', 'hard-source.txt', 'relative-link', 'broken-link', ...(didMove ? [] : ['hard-alias.txt'])]],
        [copied, didCopy ? ['hard-alias.txt'] : []], [moved, didMove ? ['hard-alias.txt'] : []]]) {
        await noLinks(base, fs); assert.deepEqual((await fs.readdir(base)).sort(), expected.sort())
      }
      if (didCopy) {
        const stat = await fs.lstat(child(copied, 'hard-alias.txt'), { bigint: true })
        assert.ok(stat.isFile()); assert.equal(stat.nlink, 1n)
        assert.ok(String(stat.dev) !== policy.hardlinks[0].device || String(stat.ino) !== policy.hardlinks[0].inode)
        assert.deepEqual(await fixture.readBytes(child(copied, 'hard-alias.txt')), hardBytes)
      }
      part.digests = [{ name: 'target.txt', bytes: targetBytes.length, sha256: sha256(targetBytes) },
        { name: 'hard-alias.txt', bytes: hardBytes.length, sha256: sha256(hardBytes) }]
      part.links = { symlinks: 2, knownHardAliases: 2, copiedIndependent: didCopy, movedSameInode: didMove }
      assert.equal((await ui.handshake(plan.runId)).cancelTasks, 0); assert.deepEqual(await transferListeners(ui), [])
    }
    const step = (id, action) => recordPart(result, id, async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; await action(part); part.ui = 'ACKNOWLEDGED'
      result.phase = 'verification'; await verify(part)
    })
    const sourcePaths = () => ['target.txt', 'hard-source.txt', 'hard-alias.txt', 'relative-link', 'broken-link'].map(name => child(source, name))
    await step('list-relative-broken', async () => {
      for (const view of ['list', 'grid']) { await ui.navigate(source); await ui.setView(view); await ui.listing(source, view, sourcePaths()) }
      await ui.setView('list')
      for (const link of policy.symlinks) {
        const row = await ui.browser.$(`[data-path=${JSON.stringify(link.path)}]`)
        assert.equal(await (await row.$('.col-type')).getText(), 'Link')
      }
    })
    for (const name of ['relative-link', 'broken-link']) await step(name === 'relative-link' ? 'relative-copy-rejection' : 'broken-copy-rejection', async part => {
      await ui.navigate(source); await ui.select(child(source, name)); await ui.chord('c')
      part.feedback = await ui.expectedToast(/Copy failed:.*Symlinks are not supported/i)
    })
    await step('hardlink-copy', async () => {
      await ui.populateClipboard(source, [child(source, 'hard-alias.txt')], false, false)
      await ui.paste(copied, child(copied, 'hard-alias.txt')); await waitActivityGone(ui); didCopy = true
    })
    await step('hardlink-move', async () => {
      await ui.setView('grid'); await ui.populateClipboard(source, [child(source, 'hard-alias.txt')], true, true)
      await ui.paste(moved, child(moved, 'hard-alias.txt'), { menu: true }); await waitActivityGone(ui); didMove = true
    })
  }, { id: 'links-local', providers: ['local'] })
}

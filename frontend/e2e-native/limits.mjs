/* global document */
import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { Key } from 'webdriverio'
import { child, noLinks, ownedPath } from './scope.mjs'
import { verifyTree } from './editing.mjs'
import { recordPart } from './report.mjs'

export function limitsManifest(plan) {
  return plan.targets.map(target => ({ id: `limits-${target.kind}`, name: `${target.kind}: names and representation limits`,
    providers: [target.kind], partIds: ['reserved-name', 'name-limit', 'rename-separator',
      ...(target.kind === 'local' ? ['utf8-byte-limit-valid', 'utf8-byte-limit-reject', 'path-limit', 'invalid-encoding-list', 'invalid-encoding-search'] : [])] }))
}
const nameError = /invalid|reserved|restricted|not allowed|cannot.*name|name.*(?:long|contain)|path.*long|too long|input\/output error/i
async function createAttempt(ui, base, name, { reject = false } = {}) {
  await ui.beginCreation(base, false)
  await ui.fill(await ui.browser.$('#new-file-name'), name)
  await ui.browser.keys([Key.Enter])
  let message
  await ui.browser.waitUntil(async () => {
    const error = await ui.browser.$('[role="dialog"] .pill.error')
    if (await error.isExisting() && await error.isDisplayed()) { message = await error.getText(); return true }
    return !await (await ui.browser.$('#new-file-name')).isExisting()
  }, { timeout: 180_000, interval: 150, timeoutMsg: 'Creation needs explicit acknowledgement or rejection' })
  if (message) {
    assert.match(message, nameError, 'Only a name/path rejection certifies this part')
    await ui.cancelCreation(false)
    return { outcome: 'EXPLICIT_REJECTION', message }
  }
  assert.ok(!reject, 'An over-limit name must not be silently truncated or accepted')
  await ui.idle({ resultPath: child(base, name) })
  return { outcome: 'EXACT_NAME_CREATED' }
}

// Only one literal invalid byte component below an already-owned local directory.
// Preserve the same inode and bytes under a UTF-8 recovery name after observation,
// including failure; never delete recovery or convert the invalid byte to U+FFFD.
async function invalidEncoding(fixture, base, label, observe) {
  ownedPath(fixture.roots, base); await noLinks(base, fs)
  const leaf = Buffer.concat([Buffer.from(`invalid-${label}-`), Buffer.from([0xff])])
  const raw = Buffer.concat([Buffer.from(`${base}/`), leaf]), retained = child(base, `retained-invalid-${label}.txt`)
  assert.ok(!await fixture.exists(retained))
  const handle = await fs.open(raw, fs.constants.O_RDWR | fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_NOFOLLOW, 0o600)
  const content = Buffer.from(`Generated unsupported encoding ${label}.\n`)
  try {
    await handle.writeFile(content)
    const original = await handle.stat({ bigint: true })
    try { await observe() }
    finally {
      const current = await fs.lstat(raw, { bigint: true })
      assert.ok(current.isFile() && current.dev === original.dev && current.ino === original.ino && current.nlink === 1n,
        'Cannot relabel a changed invalid-name fixture')
      assert.deepEqual(await fs.readFile(raw), content, 'Original invalid-name bytes must be preserved')
      assert.ok(!await fixture.exists(retained))
      await fs.rename(raw, retained)
    }
  } finally { await handle.close() }
  return { bytes: content.toString('utf8'), nameBytesHex: leaf.toString('hex'), retainedName: retained.split('/').at(-1) }
}
async function encodingError(ui) {
  let message
  await ui.browser.waitUntil(async () => {
    const error = await ui.browser.$('.notice-error')
    if (!await error.isExisting() || !await error.isDisplayed()) return false
    message = await error.getText()
    return Boolean(message)
  }, { timeout: 180_000, interval: 100, timeoutMsg: 'Expected readable filename-encoding error notice' })
  assert.match(message, /filename.*(?:not valid UTF-8|unsupported.*encoding)/i)
  const paths = await ui.browser.execute(() => [...document.querySelectorAll('[data-path]')].map(node => node.dataset.path))
  for (const raw of paths) ownedPath(ui.roots, raw)
  assert.ok(paths.every(raw => !raw.includes('\ufffd')), 'An unrepresentable name must not become a lossy actionable alias')
}

export async function limits(plan, fixture, ui, record) {
  ui.waitTimeout = 180_000; ui.transferTimeout = 600_000
  for (const target of plan.targets) await record(`${target.kind}: names and representation limits`, async result => {
    result.phase = 'setup'
    const base = child(target.files, 'limits'), expected = new Map([['sentinel.txt', 'Preserve name-limit sentinel.\n']])
    await fixture.mkdir(base); await fixture.write(child(base, 'sentinel.txt'), expected.get('sentinel.txt'))
    await ui.navigate(base); await ui.setView('list')
    const step = (id, action) => recordPart(result, id, async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; await action(part); part.ui = 'ACKNOWLEDGED'
      result.phase = 'verification'; await verifyTree(fixture, base, expected)
    })
    await step('reserved-name', async part => {
      Object.assign(part, await createAttempt(ui, base, 'CON'))
      if (part.outcome === 'EXACT_NAME_CREATED') expected.set('CON', '')
    })
    await step('name-limit', async part => { Object.assign(part, await createAttempt(ui, base, 'n'.repeat(512), { reject: true })) })
    await step('rename-separator', async () => {
      await ui.beginRename(child(base, 'sentinel.txt'))
      await ui.renameDraft('invalid/name', { error: /invalid.*name|path separators/i }); await ui.cancelRename()
    })
    if (target.kind === 'local') {
      await step('utf8-byte-limit-valid', async part => {
        const name = `${'æ'.repeat(127)}x`; assert.equal(Buffer.byteLength(name), 255)
        Object.assign(part, await createAttempt(ui, base, name)); assert.equal(part.outcome, 'EXACT_NAME_CREATED'); expected.set(name, '')
      })
      await step('utf8-byte-limit-reject', async part => {
        const name = 'æ'.repeat(128); assert.equal(Buffer.byteLength(name), 256)
        Object.assign(part, await createAttempt(ui, base, name, { reject: true }))
      })
      await recordPart(result, 'path-limit', async part => {
        result.phase = 'setup'
        const root = child(target.files, 'limits-path'), chain = [root]
        await fixture.mkdir(root)
        while (Buffer.byteLength(chain.at(-1)) + 241 <= 4000) {
          assert.ok(chain.length < 20, 'Explicit deep path fixture budget')
          const next = child(chain.at(-1), `d${'p'.repeat(239)}`)
          await fixture.mkdir(next); chain.push(next)
        }
        result.phase = 'ui'; part.ui = 'STARTED'; await ui.navigate(chain.at(-1))
        Object.assign(part, await createAttempt(ui, chain.at(-1), 'p'.repeat(240), { reject: true }))
        part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'
        for (let i = 0; i < chain.length; i++) {
          const rows = await fixture.snapshot(chain[i], { maxChildren: 1 })
          assert.deepEqual(rows.map(row => [row.path, row.kind]), i + 1 < chain.length ? [[chain[i + 1], 'dir']] : [])
        }
        part.depth = chain.length; part.parentPathBytes = Buffer.byteLength(chain.at(-1))
        await verifyTree(fixture, base, expected); await ui.navigate(base)
      })
      for (const mode of ['list', 'search']) await step(`invalid-encoding-${mode}`, async part => {
        await ui.navigate(base)
        if (mode === 'search') await ui.search(base, 'invalid', true)
        const observed = await invalidEncoding(fixture, base, mode, async () => {
          if (mode === 'list') await ui.browser.keys([Key.F5])
          else { await ui.fill(await ui.browser.$('#explorer-path-input'), 'invalid'); await ui.browser.keys([Key.Enter]) }
          await encodingError(ui)
        })
        expected.set(observed.retainedName, observed.bytes); part.nameBytesHex = observed.nameBytesHex
        // One deliberate request after the fixture is represented safely clears
        // the expected error; no failed mutation is resent.
        if (mode === 'search') {
          await ui.fill(await ui.browser.$('#explorer-path-input'), 'invalid'); await ui.browser.keys([Key.Enter])
        } else await ui.browser.keys([Key.F5])
        await ui.browser.waitUntil(async () => !await (await ui.browser.$('.notice-error')).isExisting(), { timeout: 180_000 })
        await ui.idle(); if (mode === 'search') await ui.exitQuery(base)
      })
    }
    assert.equal((await ui.handshake(plan.runId)).cancelTasks, 0)
  }, { id: `limits-${target.kind}`, providers: [target.kind] })
}

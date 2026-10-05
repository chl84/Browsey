import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Key } from 'webdriverio'
import { NativeUi } from './ui.mjs'

test('native modifier actions use WebDriver key codes and release on failure', async () => {
  const values = []
  let released = 0
  const action = { down(value) { values.push(['down', value]); return this },
    up(value) { values.push(['up', value]); return this },
    async perform() { throw new Error('synthetic delivery failure') } }
  const ui = new NativeUi({ action: () => action, releaseActions: async () => { released++ } }, [])
  await assert.rejects(ui.chord('Delete', 'Shift'), /delivery failure/)
  assert.deepEqual(values, [['down', Key.Shift], ['down', Key.Delete], ['up', Key.Delete], ['up', Key.Shift]])
  assert.equal(released, 1)
})

test('input mismatch or lost focus aborts before the caller can submit', async () => {
  const action = { down() { return this }, up() { return this }, perform: async () => {} }
  const ui = new NativeUi({ releaseActions: async () => {}, action: () => action, keys: async () => {} }, [])
  await assert.rejects(ui.fill({ click: async () => {}, isFocused: async () => true,
    setValue: async () => {}, getValue: async () => 'wrong/value' }, '/owned/input_æøå'), /intended value/)
  await assert.rejects(ui.fill({ click: async () => {}, isFocused: async () => false }, '/owned/input_æøå'), /have focus/)
})

test('an app-reported failure stops readiness even when the operation flag is idle', async () => {
  const ui = new NativeUi({ execute: async () => ({ idle: true, errors: ['Paste failed: Cloud operation failed.'] }),
    waitUntil: async predicate => assert.equal(await predicate(), true) }, [])
  await assert.rejects(ui.idle(), error => error.failureKind === 'APP_REPORTED_ERROR' && /Paste failed/.test(error.message))
})

test('a cut must be acknowledged before navigation/paste, without retrying the shortcut', async () => {
  const calls = []
  const ui = new NativeUi({ execute: async () => undefined,
    waitUntil: async predicate => assert.equal(await predicate(), true) }, ['/owned/files'])
  ui.navigate = async path => { calls.push(['navigate', path]) }
  ui.select = async () => {}
  ui.chord = async key => { calls.push(['key', key]) }
  ui.idle = async expected => {
    assert.deepEqual(expected, { toast: 'Cut', cutPath: '/owned/files/source/sample.txt' })
    throw new Error('Cut acknowledgement absent')
  }
  await assert.rejects(ui.transfer('/owned/files/source/sample.txt', '/owned/files/dest', true), /acknowledgement absent/)
  assert.deepEqual(calls, [['navigate', '/owned/files/source'], ['key', 'x']])
})

test('navigation helpers reject outside history/breadcrumb/folder/listing paths before interacting with the window', async () => {
  const ui = new NativeUi({}, ['/owned/files'])
  for (const outside of ['/personal', '/owned/files-sibling', '/owned/files/../outside']) {
    await assert.rejects(ui.openFolder(outside))
    await assert.rejects(ui.breadcrumb(outside))
    await assert.rejects(ui.history('back', outside))
    await assert.rejects(ui.waitPath(outside))
    await assert.rejects(ui.listing('/owned/files', 'list', [outside]))
  }
  await assert.rejects(ui.setView('unknown'))
})

test('listing stops immediately on an outside observation and rejects unexpected saved bookmarks', async () => {
  let polls = 0
  const ui = new NativeUi({ execute: async () => ({ current: '/personal', view: 'list', paths: [] }),
    waitUntil: async predicate => { polls++; assert.equal(await predicate(), true) } }, ['/owned/files'])
  ui.idle = async () => {}
  await assert.rejects(ui.listing('/owned/files', 'list', []), /outside/)
  assert.equal(polls, 1)
  ui.browser.execute = async () => ['/owned/files', '/personal']
  await assert.rejects(ui.bookmarks(), /outside/)
})

test('view controls close an unchanged-mode menu and accept a changed-mode menu that closes itself', async () => {
  for (const checked of ['false', 'true']) {
    let overlay = false, closes = 0, toggles = 0
    const browser = { $: async selector => ({
      click: async () => {
        if (selector.includes('Main menu')) overlay = true
        else if (selector.includes('role="switch"')) { toggles++; overlay = false }
        else if (selector === '.menu-overlay') { closes++; overlay = false }
      },
      getAttribute: async () => checked, waitForDisplayed: async () => {},
      isExisting: async () => overlay, waitForExist: async () => assert.equal(overlay, false),
    }) }
    const ui = new NativeUi(browser, [])
    ui.idle = async () => {}
    await ui.setView('list')
    assert.equal(toggles, checked === 'true' ? 1 : 0)
    assert.equal(closes, checked === 'false' ? 1 : 0)
  }
})

test('menu refresh clicks the owned main action and waits for its overlay to close', async () => {
  const calls = []
  const ui = new NativeUi({ $: async selector => ({
    click: async () => { calls.push(selector) },
    waitForDisplayed: async () => {},
    waitForExist: async options => { assert.equal(options.reverse, true); calls.push('closed') },
  }) }, [])
  ui.idle = async () => { calls.push('idle') }
  await ui.menuRefresh()
  assert.deepEqual(calls, ['idle', '[aria-label="Main menu"]',
    '//*[@role="menu" and @aria-label="Main actions"]//button[@role="menuitem" and normalize-space(.)="Refresh"]', 'closed', 'idle'])
})

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Key } from 'webdriverio'
import { NativeUi } from './ui.mjs'

test('Properties reads visible children of display-contents rows and excludes hidden tabs', async () => {
  const original = globalThis.document
  const row = (label, value, visible) => ({ getClientRects: () => [], querySelector: selector => ({
    textContent: selector === '.label' ? label : value, getClientRects: () => visible ? [{}] : [],
  }) })
  globalThis.document = { querySelectorAll: () => [row('Size', '42 B\n (1 item)', true), row('Size', 'stale hidden tab', false)] }
  try {
    const ui = new NativeUi({ execute: async action => action() }, [])
    assert.deepEqual(await ui.propertiesRows(), { Size: '42 B (1 item)' })
  } finally { globalThis.document = original }
})

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

test('deliberate repeated input is bounded and releases modifiers after a failed delivery', async () => {
  let releases = 0, deliveries = 0
  const values = []
  const action = { down(value) { values.push(['down', value]); return this }, up(value) { values.push(['up', value]); return this },
    perform: async () => { deliveries++; throw Error('delivery failed') } }
  const ui = new NativeUi({ action: () => action, releaseActions: async () => { releases++ } }, ['/owned/files'])
  await assert.rejects(ui.burst('v', 0)); await assert.rejects(ui.burst('v', 4))
  assert.equal(deliveries, 0)
  await assert.rejects(ui.burst('v', 3), /delivery failed/)
  assert.equal(deliveries, 1); assert.equal(releases, 1)
  assert.equal(values.filter(([operation, value]) => operation === 'down' && value === 'v').length, 3)
  assert.deepEqual(values[0], ['down', Key.Control]); assert.deepEqual(values.at(-1), ['up', Key.Control])
})

test('modified selection rejects outside paths before input and releases a held modifier if click fails', async () => {
  let released = 0, clicked = 0
  const action = { down() { return this }, up() { return this }, perform: async () => {} }
  const ui = new NativeUi({ releaseActions: async () => { released++ }, action: () => action,
    $: async () => ({ waitForDisplayed: async () => {}, click: async () => { clicked++; throw new Error('synthetic pointer failure') } }) }, ['/owned/files'])
  await assert.rejects(ui.modifiedSelect('/outside', 'Control'), /outside/)
  assert.equal(released, 0); assert.equal(clicked, 0)
  await assert.rejects(ui.modifiedSelect('/owned/files/generated.txt', 'Shift'), /pointer failure/)
  assert.equal(clicked, 1); assert.equal(released, 2)
})

test('selected copy stops after one unacknowledged request without navigating or pasting', async () => {
  const calls = []
  const ui = new NativeUi({ execute: async () => undefined,
    waitUntil: async predicate => assert.equal(await predicate(), true) }, ['/owned/files'])
  ui.chord = async key => { calls.push(key) }
  ui.navigate = async () => { calls.push('navigate') }
  ui.idle = async () => { throw new Error('synthetic copy acknowledgement absent') }
  await assert.rejects(ui.copySelection('/owned/files/dest', ['/owned/files/source/a.txt']), /acknowledgement absent/)
  assert.deepEqual(calls, ['c'])
})

test('acknowledged selected copy submits one copy and one paste in order', async () => {
  const calls = []
  const ui = new NativeUi({ execute: async () => undefined,
    waitUntil: async predicate => assert.equal(await predicate(), true) }, ['/owned/files'])
  ui.chord = async key => { calls.push(key) }
  ui.navigate = async dest => { calls.push(['navigate', dest]) }
  ui.idle = async expected => { calls.push(expected) }
  await ui.copySelection('/owned/files/dest', ['/owned/files/source/a.txt', '/owned/files/source/c.txt'])
  assert.deepEqual(calls, ['c', { toast: 'Copied' }, ['navigate', '/owned/files/dest'], 'v',
    { resultPath: '/owned/files/dest/a.txt' }])
})

test('selection rejects duplicate expected entries and an outside observation without swallowed retries', async () => {
  let polls = 0
  const ui = new NativeUi({ execute: async () => ({ current: '/owned/files', rows: [{ path: '/outside', selected: true }], text: '' }),
    waitUntil: async predicate => { polls++; assert.equal(await predicate(), true) } }, ['/owned/files'])
  ui.idle = async () => {}
  await assert.rejects(ui.selection('/owned/files', ['/owned/files/a', '/owned/files/a']), /duplicates/)
  assert.equal(polls, 0)
  await assert.rejects(ui.selection('/owned/files', []), /outside/)
  assert.equal(polls, 1)
})

test('input mismatch or lost focus aborts before the caller can submit', async () => {
  const action = { down() { return this }, up() { return this }, perform: async () => {} }
  const ui = new NativeUi({ releaseActions: async () => {}, action: () => action, keys: async () => {} }, [])
  await assert.rejects(ui.fill({ click: async () => {}, isFocused: async () => true,
    setValue: async () => {}, getValue: async () => 'wrong/value' }, '/owned/input_æøå'), /intended value/)
  await assert.rejects(ui.fill({ click: async () => {}, isFocused: async () => false }, '/owned/input_æøå'), /have focus/)
})

test('nonempty input replacement does not exit a one-character filter through Backspace', async () => {
  const keys = []
  const action = { down() { return this }, up() { return this }, perform: async () => {} }
  const ui = new NativeUi({ releaseActions: async () => {}, action: () => action,
    keys: async values => { keys.push(...values) } }, [])
  await ui.fill({ click: async () => {}, isFocused: async () => true, getValue: async () => 'ALPHA' }, 'ALPHA')
  assert.ok(!keys.includes(Key.Backspace))
})

test('Norwegian slash input explicitly presses/releases Shift+7 without changing other layouts', async () => {
  for (const inputLayout of ['no', 'us']) {
    const batches = []
    const browser = { releaseActions: async () => {}, keys: async () => {}, action: () => {
      const values = []
      return { down(value) { values.push(['down', value]); return this },
        up(value) { values.push(['up', value]); return this }, perform: async () => { batches.push(values) } }
    } }
    const ui = new NativeUi(browser, [], { inputLayout })
    await ui.fill({ click: async () => {}, isFocused: async () => true, getValue: async () => '/' }, '/')
    assert.deepEqual(batches.at(-1), inputLayout === 'no'
      ? [['down', Key.Shift], ['down', '7'], ['up', '7'], ['up', Key.Shift]]
      : [['down', '/'], ['up', '/']])
  }
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
    await assert.rejects(ui.listing('/owned/files', 'list', [], { fileOrder: [outside] }))
    await assert.rejects(ui.search(outside, 'alpha', true))
    await assert.rejects(ui.filter('/owned/files', outside, 'alpha'))
    await assert.rejects(ui.exitQuery(outside))
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

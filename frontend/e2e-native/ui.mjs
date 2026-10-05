/* global document, window */
import assert from 'node:assert/strict'
import { Key } from 'webdriverio'
import { child, ownedPath, inside } from './scope.mjs'

// One UI driver and shared cases, not a second set of file-operation semantics.
// Mutations go through real Browsey controls/shortcuts, never direct IPC calls.
export class NativeUi {
  constructor(browser, roots, { inputLayout = process.env.BROWSEY_NATIVE_INPUT_LAYOUT } = {}) {
    this.browser = browser; this.roots = roots; this.inputLayout = inputLayout
  }

  async handshake(runId) {
    const status = await this.browser.execute(async () => {
      return window.__TAURI_INTERNALS__.invoke('native_test_status')
    })
    assert.equal(status.runId, runId, 'Refuse a production/other-session window')
    assert.equal(status.scope, 'owned-files-only')
    assert.ok(Number.isSafeInteger(status.pid) && status.pid > 1)
    return status
  }

  async idle(expected = {}, timeout = 60_000) {
    let reportedFailure
    await this.browser.waitUntil(async () => {
      const state = await this.browser.execute(expected => {
        const visible = node => node.getClientRects().length > 0
        const errors = [...document.querySelectorAll('.pill.error')].filter(visible).map(node => node.textContent.trim())
        for (const node of [...document.querySelectorAll('.toast[role="status"]')].filter(visible)) {
          const message = node.textContent.trim()
          if (/^(?:(?:paste|copy|cut|move(?: to trash)?|rename|delete|create|undo|redo|refresh|load|navigation|search)\s+failed\b|failed to\b|could not\b|error\b)/i.test(message)) errors.push(message)
        }
        const rows = [...document.querySelectorAll('[data-path]')].filter(visible)
        const acknowledged = (!expected.toast || [...document.querySelectorAll('.toast[role="status"]')]
          .some(node => visible(node) && node.textContent.trim() === expected.toast))
          && (!expected.cutPath || rows.some(node => node.dataset.path === expected.cutPath && node.classList.contains('cut')))
          && (!expected.resultPath || rows.some(node => node.dataset.path === expected.resultPath))
          && (!expected.absentPath || !rows.some(node => node.dataset.path === expected.absentPath))
        return { errors, idle: document.querySelector('main.shell[aria-busy="false"][data-operation-active="false"]') !== null
          && ![...document.querySelectorAll('[role="dialog"]')].some(visible) && acknowledged }
      }, expected)
      if (state.errors.length) {
        reportedFailure = Object.assign(new Error(`Browsey reported failure: ${state.errors.join('; ')}`), { failureKind: 'APP_REPORTED_ERROR' })
        // Resolve the readiness wait, then throw outside it so WebDriver does
        // not swallow/retry the predicate error until the timeout expires.
        return true
      }
      return state.idle
    }, { timeout, interval: 150, timeoutMsg: 'Browsey did not finish without an error/dialog' })
    if (reportedFailure) throw reportedFailure
  }

  async navigate(raw) {
    ownedPath(this.roots, raw)
    await this.idle()
    await this.browser.releaseActions()
    const root = this.roots.find(root => raw === root || raw.startsWith(`${root}/`))
    const bookmark = await this.browser.$(`.bookmark[data-drop-path=${JSON.stringify(root)}]`)
    await bookmark.waitForDisplayed({ timeout: 10_000 })
    await bookmark.click()
    await this.waitPath(root)
    // Watchers are intentionally disabled. Refresh fixture setup through the UI.
    await this.refresh()
    let current = root
    for (const part of raw.slice(root.length + 1).split('/').filter(Boolean)) {
      current = child(current, part)
      await this.select(current)
      await this.browser.keys(['Enter'])
      await this.waitPath(current)
    }
  }

  async waitPath(raw) {
    ownedPath(this.roots, raw)
    await this.browser.waitUntil(async () => await (await this.browser.$('main.shell')).getAttribute('data-current-path') === raw,
      { timeout: 60_000, timeoutMsg: 'UI did not navigate to the owned fixture folder' })
    await this.idle()
  }

  async setView(view) {
    assert.ok(['list', 'grid'].includes(view), 'Expected list or grid view')
    await this.idle()
    await (await this.browser.$('[aria-label="Main menu"]')).click()
    const toggle = await this.browser.$('[role="switch"][aria-label="Toggle list or grid view"]')
    await toggle.waitForDisplayed({ timeout: 5000 })
    if (await toggle.getAttribute('aria-checked') !== String(view === 'grid')) await toggle.click()
    // If the current mode already matches, focus stays on the menu opener;
    // Escape would not reach the menu's key handler. Close via its real overlay.
    const overlay = await this.browser.$('.menu-overlay')
    if (await overlay.isExisting()) await overlay.click()
    await overlay.waitForExist({ reverse: true, timeout: 5000 })
    await (await this.browser.$(view === 'list' ? '.rows' : '.grid')).waitForDisplayed({ timeout: 5000 })
    await this.idle()
  }

  async openFolder(raw) {
    ownedPath(this.roots, raw)
    await this.idle()
    await this.browser.releaseActions()
    await this.select(raw)
    await this.browser.keys([Key.Enter])
    await this.waitPath(raw)
  }

  async history(direction, expected) {
    ownedPath(this.roots, expected)
    assert.ok(['back', 'forward'].includes(direction), 'Expected back or forward')
    await this.idle()
    await (await this.browser.$(`[aria-label="Go ${direction}"]`)).click()
    await this.waitPath(expected)
  }

  async breadcrumb(raw) {
    ownedPath(this.roots, raw)
    await this.idle()
    const crumb = await this.browser.$(`.crumb[data-drop-path=${JSON.stringify(raw)}]`)
    await crumb.waitForDisplayed({ timeout: 5000 })
    await crumb.click()
    await this.waitPath(raw)
  }

  async bookmarks() {
    const paths = await this.browser.execute(() => [...document.querySelectorAll('.bookmark[data-drop-path]')]
      .map(node => node.dataset.dropPath))
    for (const raw of paths) ownedPath(this.roots, raw)
    assert.deepEqual(paths.sort(), [...this.roots].sort(), 'Only this session\'s owned bookmarks may be loaded')
  }

  async listing(raw, view, paths, { fileOrder } = {}) {
    ownedPath(this.roots, raw)
    assert.ok(['list', 'grid'].includes(view))
    for (const path of paths) ownedPath(this.roots, path)
    if (fileOrder) for (const path of fileOrder) {
      ownedPath(this.roots, path)
      assert.ok(paths.includes(path), 'Ordered files must belong to the expected listing')
    }
    const expected = [...paths].sort()
    await this.idle()
    let state
    await this.browser.waitUntil(async () => {
      state = await this.browser.execute(() => {
        const visible = node => node.getClientRects().length > 0
        const collection = [...document.querySelectorAll('.rows, .grid')].find(visible)
        return { current: document.querySelector('main.shell')?.dataset.currentPath,
          view: collection?.classList.contains('grid') ? 'grid' : 'list',
          paths: collection ? [...collection.querySelectorAll('[data-path]')].filter(visible).map(node => node.dataset.path) : [],
          empty: collection?.textContent.includes('No items here.') ?? false }
      })
      // Resolve immediately on an outside observation, then throw outside the
      // WebDriver predicate so a scope failure cannot be swallowed/retried.
      if ([state.current, ...state.paths].filter(Boolean).some(path => !this.roots.some(root => inside(root, path)))) return true
      return state.current === raw && state.view === view && JSON.stringify([...state.paths].sort()) === JSON.stringify(expected)
        && (!fileOrder || JSON.stringify(state.paths.filter(path => fileOrder.includes(path))) === JSON.stringify(fileOrder))
        && (expected.length > 0 || state.empty)
    }, { timeout: 60_000, interval: 150, timeoutMsg: 'Owned folder contents/view did not match after navigation' })
    ownedPath(this.roots, state.current)
    for (const path of state.paths) ownedPath(this.roots, path)
    assert.equal(state.current, raw)
    assert.equal(state.view, view)
    assert.deepEqual([...state.paths].sort(), expected)
    if (fileOrder) assert.deepEqual(state.paths.filter(path => fileOrder.includes(path)), fileOrder)
    if (!expected.length) assert.ok(state.empty, 'Empty folder must show its empty state')
  }

  async select(raw) {
    ownedPath(this.roots, raw)
    const row = await this.browser.$(`[data-path=${JSON.stringify(raw)}]`)
    await row.waitForDisplayed({ timeout: 60_000 })
    await row.click()
    return row
  }

  async modifiedSelect(raw, modifier) {
    ownedPath(this.roots, raw)
    assert.ok(['Control', 'Shift'].includes(modifier))
    await this.browser.releaseActions()
    const row = await this.browser.$(`[data-path=${JSON.stringify(raw)}]`)
    await row.waitForDisplayed({ timeout: 5000 })
    try {
      await this.browser.action('key').down(Key[modifier]).perform(true)
      await row.click({ button: 'left', skipRelease: true })
      await this.browser.action('key').up(Key[modifier]).perform()
    } finally { await this.browser.releaseActions() }
  }

  async selection(raw, paths, { directories = [] } = {}) {
    ownedPath(this.roots, raw)
    for (const path of [...paths, ...directories]) ownedPath(this.roots, path)
    assert.equal(new Set(paths).size, paths.length, 'Expected selection cannot contain duplicates')
    assert.ok(directories.every(path => paths.includes(path)))
    await this.idle()
    let state
    await this.browser.waitUntil(async () => {
      state = await this.browser.execute(() => {
        const collection = [...document.querySelectorAll('.rows, .grid')].find(node => node.getClientRects().length)
        return { current: document.querySelector('main.shell')?.dataset.currentPath,
          rows: [...(collection?.querySelectorAll('[data-path]') ?? [])].map(node => ({ path: node.dataset.path,
            selected: node.classList.contains('selected') })),
          text: document.querySelector('.statusbar .status-text')?.textContent ?? '' }
      })
      if ([state.current, ...state.rows.map(row => row.path)].filter(Boolean).some(path => !this.roots.some(root => inside(root, path)))) return true
      const counts = noun => Number(state.text.match(new RegExp(`(?:^|\\|\\s*)([0-9]+) ${noun}s? selected`))?.[1] ?? 0)
      return state.current === raw && state.rows.every(row => row.selected === paths.includes(row.path))
        && counts('file') === paths.length - directories.length && counts('folder') === directories.length
    }, { timeout: 10_000, interval: 100, timeoutMsg: 'Rendered selection/status count did not match the intended entries' })
    ownedPath(this.roots, state.current)
    for (const row of state.rows) ownedPath(this.roots, row.path)
  }

  async selectionKey(key, modifier) {
    assert.ok(['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'Escape'].includes(key))
    if (modifier) await this.chord(key, modifier)
    else { await this.browser.releaseActions(); await this.browser.keys([Key[key]]) }
    await this.idle()
  }

  async emptySpace(raw) {
    await this.waitPath(raw)
    // Inspect hit testing only; selection is changed by a real pointer click.
    const point = await this.browser.execute(() => {
      const collection = [...document.querySelectorAll('.rows, .grid')].find(node => node.getClientRects().length)
      const rect = collection.getBoundingClientRect()
      for (let y = Math.floor(rect.bottom - 20); y > rect.top + 20; y -= 16) {
        const x = Math.floor(rect.left + rect.width / 2)
        if (document.elementFromPoint(x, y) === collection) return { x, y }
      }
      return null
    })
    assert.ok(point, 'Expected a hittable empty collection background')
    try { await this.browser.action('pointer').move({ origin: 'viewport', ...point }).down().up().perform() }
    finally { await this.browser.releaseActions() }
  }

  async copySelection(dest, paths) {
    ownedPath(this.roots, dest)
    assert.ok(paths.length)
    for (const path of paths) ownedPath(this.roots, path)
    await this.browser.waitUntil(async () => await this.browser.execute(() =>
      document.querySelector('.toast[role="status"]')?.textContent.trim()) !== 'Copied', { timeout: 5000 })
    await this.chord('c')
    await this.idle({ toast: 'Copied' })
    await this.navigate(dest)
    await this.chord('v')
    await this.idle({ resultPath: child(dest, paths[0].split('/').at(-1)) }, 360_000)
  }

  async virtualWindow(raw, allPaths, required) {
    ownedPath(this.roots, raw)
    for (const path of allPaths) ownedPath(this.roots, path)
    assert.ok(allPaths.includes(required))
    let state
    await this.browser.waitUntil(async () => {
      state = await this.browser.execute(() => {
        const collection = document.querySelector('.rows')
        return { current: document.querySelector('main.shell')?.dataset.currentPath,
          paths: [...collection.querySelectorAll('[data-path]')].map(node => node.dataset.path),
          height: collection.clientHeight, total: collection.scrollHeight }
      })
      if ([state.current, ...state.paths].some(path => !this.roots.some(root => inside(root, path)))) return true
      return state.current === raw && state.paths.includes(required)
    }, { timeout: 10_000, interval: 100 })
    ownedPath(this.roots, state.current)
    for (const path of state.paths) ownedPath(this.roots, path)
    assert.ok(state.paths.length > 0 && state.paths.length < allPaths.length, 'Large fixture must actually be virtualized')
    assert.ok(state.paths.every(path => allPaths.includes(path)), 'Virtual window must contain only generated entries')
    assert.ok(state.total > state.height, 'Virtual fixture must exceed the viewport')
  }

  async arrowSteps(count, direction = 'ArrowDown') {
    assert.ok(Number.isSafeInteger(count) && count > 0 && count <= 256)
    assert.ok(['ArrowDown', 'ArrowUp'].includes(direction))
    const action = this.browser.action('key')
    for (let i = 0; i < count; i++) action.down(Key[direction]).up(Key[direction])
    try { await action.perform() } finally { await this.browser.releaseActions() }
    await this.idle()
  }

  async chord(key, modifier = 'Control') {
    // Explicit key-up ordering and action release avoid sticky modifiers in
    // native WebKit (notably Shift+Delete followed by typing a slash).
    const modifierValue = Key[modifier]
    const keyValue = Key[key] ?? key
    assert.ok(modifierValue, 'Expected a known WebDriver modifier')
    try {
      await this.browser.action('key').down(modifierValue).down(keyValue).up(keyValue).up(modifierValue).perform()
    } finally { await this.browser.releaseActions() }
  }

  async refresh() { await this.browser.keys([Key.F5]); await this.idle() }

  async menuRefresh() {
    await this.idle()
    await (await this.browser.$('[aria-label="Main menu"]')).click()
    const action = await this.browser.$('//*[@role="menu" and @aria-label="Main actions"]//button[@role="menuitem" and normalize-space(.)="Refresh"]')
    await action.waitForDisplayed({ timeout: 5000 })
    await action.click()
    await (await this.browser.$('.menu-overlay')).waitForExist({ reverse: true, timeout: 5000 })
    await this.idle()
  }

  async fill(input, value) {
    await this.browser.releaseActions()
    await input.click()
    assert.ok(await input.isFocused(), 'Expected the owned input to have focus')
    await this.chord('a')
    // Replace the selection directly. Backspace on a one-character folder
    // filter intentionally exits that mode and blurs the field.
    if (!value) await this.browser.keys([Key.Backspace])
    const action = this.browser.action('key')
    for (const character of value) {
      if (character === '_') action.down(Key.Shift).down('-').up('-').up(Key.Shift)
      else if (character === '/' && this.inputLayout === 'no') action.down(Key.Shift).down('7').up('7').up(Key.Shift)
      else if (/^[A-Z]$/.test(character)) action.down(Key.Shift).down(character.toLowerCase()).up(character.toLowerCase()).up(Key.Shift)
      else action.down(character).up(character)
    }
    try { await action.perform() } finally { await this.browser.releaseActions() }
    assert.equal(await input.getValue(), value, 'Native text input differed from the intended value')
    assert.ok(await input.isFocused(), 'Input lost focus before submission')
  }

  async sort(field, direction) {
    assert.ok(['Name', 'Type', 'Modified', 'Size'].includes(field))
    assert.ok(['asc', 'desc'].includes(direction))
    const header = await this.browser.$(`//*[@role="columnheader"][.//button[@aria-label="Filter ${field}"]]`)
    const expected = direction === 'asc' ? 'ascending' : 'descending'
    const current = await header.getAttribute('aria-sort')
    const clicks = current === expected ? 0 : current === 'none' && direction === 'desc' ? 2 : 1
    for (let index = 0; index < clicks; index++) {
      await (await header.$('button.header-btn')).click()
      const next = index === 0 && clicks === 2 ? 'ascending' : expected
      await this.browser.waitUntil(async () => await header.getAttribute('aria-sort') === next, { timeout: 5000 })
      await this.idle()
    }
  }

  async columnFilter(field, label, checked = true) {
    assert.ok(['Name', 'Type', 'Modified', 'Size'].includes(field))
    assert.ok(!label.includes('"'), 'Expected a generated filter label')
    await (await this.browser.$(`[aria-label="Filter ${field}"]`)).click()
    const option = await this.browser.$(`//div[contains(@class,"filter-card")]//label[.//span[contains(@class,"text") and normalize-space(.)="${label}"]]`)
    await option.waitForDisplayed({ timeout: 60_000 })
    const input = await option.$('input[type="checkbox"]')
    if (await input.isSelected() !== checked) await option.click()
    await this.browser.waitUntil(async () => await input.isSelected() === checked, { timeout: 5000 })
    await this.browser.keys([Key.Escape])
    await (await this.browser.$('.filter-layer')).waitForExist({ reverse: true, timeout: 5000 })
    await this.idle()
  }

  async resetColumn(field) {
    assert.ok(['Name', 'Type', 'Modified', 'Size'].includes(field))
    const button = await this.browser.$(`[aria-label="Filter ${field}"]`)
    assert.ok((await button.getAttribute('class')).split(' ').includes('active'))
    await button.click({ button: 'right' })
    const reset = await this.browser.$('[role="menuitem"][data-action-id="reset"]')
    await reset.waitForDisplayed({ timeout: 5000 })
    await reset.click()
    await this.idle()
  }

  async resetColumns() {
    await (await this.browser.$('[aria-label="Reset column filters"]')).click()
    await (await this.browser.$('.grid-filter-indicator')).waitForExist({ reverse: true, timeout: 5000 })
    await this.idle()
  }

  async hidden(value) {
    await (await this.browser.$('[aria-label="Main menu"]')).click()
    const toggle = await this.browser.$('//*[@role="menuitemcheckbox"][.//span[normalize-space(.)="Show Hidden Files"]]')
    await toggle.waitForDisplayed({ timeout: 5000 })
    if (await toggle.getAttribute('aria-checked') !== String(value)) await toggle.click()
    const overlay = await this.browser.$('.menu-overlay')
    if (await overlay.isExisting()) await overlay.click()
    await overlay.waitForExist({ reverse: true, timeout: 5000 })
    await this.idle()
  }

  async filter(raw, seed, query) {
    ownedPath(this.roots, raw); ownedPath(this.roots, seed)
    await this.waitPath(raw)
    await this.select(seed)
    await this.browser.keys(['a'])
    const input = await this.browser.$('#explorer-path-input')
    await this.browser.waitUntil(async () => await input.isFocused(), { timeout: 5000 })
    await this.query(query, false)
  }

  async search(raw, query, recursive) {
    ownedPath(this.roots, raw)
    assert.equal(recursive, true, 'Search must recurse on all declared providers')
    await this.waitPath(raw)
    await (await this.browser.$('[aria-label="Main menu"]')).click()
    const action = await this.browser.$('//*[@role="menu" and @aria-label="Main actions"]//button[@role="menuitem"][.//span[normalize-space(.)="Search"]]')
    await action.waitForDisplayed({ timeout: 5000 })
    await action.click()
    await (await this.browser.$('.menu-overlay')).waitForExist({ reverse: true, timeout: 5000 })
    const input = await this.browser.$('#explorer-path-input')
    await this.browser.waitUntil(async () => await input.getAttribute('aria-label') === (recursive ? 'Search' : 'Path'), { timeout: 5000 })
    await this.query(query, recursive)
  }

  async query(value, submit) {
    await this.fill(await this.browser.$('#explorer-path-input'), value)
    if (submit) await this.browser.keys([Key.Enter])
    await this.idle()
  }

  async exitQuery(raw) {
    ownedPath(this.roots, raw)
    // Blur through a real control before Escape so the global mode handler runs.
    await (await this.browser.$('[aria-label="Main menu"]')).click()
    await (await this.browser.$('.menu-overlay')).click()
    await this.browser.keys([Key.Escape])
    const input = await this.browser.$('#explorer-path-input')
    await this.browser.waitUntil(async () => await input.getAttribute('aria-label') === 'Path' && await input.getValue() === raw,
      { timeout: 60_000, timeoutMsg: 'Search/filter did not return to the same folder address' })
    await this.waitPath(raw)
  }

  async enterPath(raw) {
    ownedPath(this.roots, raw)
    await this.idle()
    const input = await this.browser.$('#explorer-path-input')
    // Long breadcrumbs can cover the field's edge. Inspect hit testing, then
    // deliver a real pointer click to an exposed part of this exact input.
    const point = await this.browser.execute(() => {
      const field = document.querySelector('#explorer-path-input')
      const rect = field.getBoundingClientRect()
      const y = Math.floor(rect.top + rect.height / 2)
      for (let x = Math.ceil(rect.left); x < Math.min(rect.right, window.innerWidth); x++) {
        if (document.elementFromPoint(x, y) === field) return { x, y }
      }
      return null
    })
    assert.ok(point, 'Expected an exposed address-field click target')
    await this.browser.action('pointer').move({ origin: 'viewport', ...point }).down().up().perform()
    await input.waitForDisplayed({ timeout: 5000 })
    await this.fill(input, raw)
    await this.browser.keys(['Enter'])
    await this.waitPath(raw)
  }

  async transfer(src, dest, move = false) {
    await this.navigate(src.slice(0, src.lastIndexOf('/')))
    await this.select(src)
    // Clipboard population awaits backend validation (notably slow on MTP).
    // First let any old acknowledgement disappear, then observe this request's
    // real toast/cut styling before navigating or pasting. Never resend it.
    await this.browser.waitUntil(async () => !['Copied', 'Cut'].includes(await this.browser.execute(() =>
      document.querySelector('.toast[role="status"]')?.textContent.trim())),
    { timeout: 5000, timeoutMsg: 'Previous clipboard acknowledgement did not disappear' })
    await this.chord(move ? 'x' : 'c')
    await this.idle({ toast: move ? 'Cut' : 'Copied', ...(move ? { cutPath: src } : {}) })
    await this.navigate(dest)
    await this.chord('v')
    // Paste preflight can await I/O before setting the operation flag. An idle
    // frame alone does not prove this request completed.
    // The provider's bounded copy/move command allows 300s, followed by listing
    // reconciliation. Keep observing this one request, including late errors;
    // a shorter UI deadline can stop a legitimate in-flight cloud transfer.
    await this.idle({ resultPath: child(dest, src.slice(src.lastIndexOf('/') + 1)) }, 360_000)
  }

  async create(base, name, folder) {
    await this.navigate(base)
    // Empty-area context menu; stable list wrapper, not desktop coordinates.
    const list = await this.browser.$('.rows')
    const size = await list.getSize()
    // WebDriver offsets are relative to the element's visible centre.
    await list.click({ button: 'right', x: 0, y: Math.max(0, Math.floor(size.height / 2) - 16) })
    const action = await this.browser.$(`[role="menuitem"][data-action-id="${folder ? 'new-folder' : 'new-file'}"]`)
    await action.waitForDisplayed({ timeout: 5000 })
    await action.click()
    const input = await this.browser.$(folder ? '#new-folder-name' : '#new-file-name')
    await input.waitForDisplayed({ timeout: 5000 })
    await this.fill(input, name)
    await this.browser.keys(['Enter'])
    await this.idle()
    await this.select(child(base, name))
  }

  async beginCreation(base, folder) {
    ownedPath(this.roots, base)
    await this.emptySpace(base)
    const collection = await this.browser.$('.rows, .grid')
    const size = await collection.getSize()
    await collection.click({ button: 'right', x: 0, y: Math.max(0, Math.floor(size.height / 2) - 16) })
    const action = await this.browser.$(`[role="menuitem"][data-action-id="${folder ? 'new-folder' : 'new-file'}"]`)
    await action.waitForDisplayed({ timeout: 5000 })
    await action.click()
    const input = await this.browser.$(folder ? '#new-folder-name' : '#new-file-name')
    await input.waitForDisplayed({ timeout: 5000 })
    await this.browser.waitUntil(async () => await input.isFocused(), { timeout: 5000 })
    assert.equal(await input.getValue(), folder ? 'New folder' : '', 'A fresh creation dialog must reset its name')
    return input
  }

  async creationValue(folder, value) {
    assert.ok(typeof value === 'string' && value.length <= 128 && !value.includes('\0'))
    await this.fill(await this.browser.$(folder ? '#new-folder-name' : '#new-file-name'), value)
  }

  async submitCreation(folder, { button = false, error } = {}) {
    const input = await this.browser.$(folder ? '#new-folder-name' : '#new-file-name')
    assert.ok(await input.isFocused(), 'Expected the intended creation input before submission')
    if (button) await (await this.browser.$('//*[@role="dialog"]//button[normalize-space(.)="Create"]')).click()
    else await this.browser.keys([Key.Enter])
    if (error) {
      const pill = await this.browser.$('[role="dialog"] .pill.error')
      await pill.waitForDisplayed({ timeout: 60_000 })
      assert.match(await pill.getText(), error, 'Creation must explain the expected rejection')
      assert.ok(await input.isDisplayed(), 'Rejected creation must keep its dialog open')
    } else await this.idle()
  }

  async cancelCreation(folder, escape = false) {
    const input = await this.browser.$(folder ? '#new-folder-name' : '#new-file-name')
    if (escape) {
      assert.ok(await input.isFocused(), 'First Escape must start in the creation input')
      await this.browser.keys([Key.Escape])
      assert.ok(await input.isDisplayed(), 'First Escape blurs a text input before closing its modal')
      assert.ok(!await input.isFocused(), 'First Escape must blur the text input')
      await this.browser.keys([Key.Escape])
    } else await (await this.browser.$('//*[@role="dialog"]//button[normalize-space(.)="Cancel"]')).click()
    await this.idle()
  }

  async creationFocus(base) {
    await this.waitPath(base)
    await this.browser.waitUntil(async () => await this.browser.execute(() => {
      const collection = document.querySelector('.rows, .grid')
      return collection && collection === document.activeElement
    }), { timeout: 5000, timeoutMsg: 'Closing creation did not restore focus to its collection trigger' })
  }

  async rename(raw, name) {
    await this.navigate(raw.slice(0, raw.lastIndexOf('/')))
    await this.select(raw)
    await this.chord('r')
    const input = await this.browser.$('#rename-entry-name')
    await input.waitForDisplayed({ timeout: 5000 })
    await this.fill(input, name)
    await this.browser.keys(['Enter'])
    await this.idle()
  }

  async menuAction(id, raw) {
    assert.ok(['rename', 'copy', 'cut', 'paste', 'delete-permanent', 'properties'].includes(id))
    const selected = raw ? await this.browser.execute(() => [...document.querySelectorAll('.rows [data-path].selected, .grid [data-path].selected')].map(node => node.dataset.path).sort()) : null
    if (raw) {
      ownedPath(this.roots, raw)
      await (await this.browser.$(`[data-path=${JSON.stringify(raw)}]`)).click({ button: 'right' })
    } else {
      const collection = await this.browser.$('.rows, .grid')
      const size = await collection.getSize()
      await collection.click({ button: 'right', x: 0, y: Math.max(0, Math.floor(size.height / 2) - 16) })
    }
    const action = await this.browser.$(`[role="menuitem"][data-action-id="${id}"]`)
    await action.waitForDisplayed({ timeout: 5000 })
    if (selected?.includes(raw)) assert.deepEqual(await this.browser.execute(() => [...document.querySelectorAll('.rows [data-path].selected, .grid [data-path].selected')].map(node => node.dataset.path).sort()), selected,
      'Opening a selected-entry context menu must preserve the full selection')
    await action.click()
  }

  async beginRename(raw, menu = false) {
    ownedPath(this.roots, raw)
    await this.select(raw)
    if (menu) await this.menuAction('rename', raw)
    else await this.chord('r')
    const input = await this.browser.$('#rename-entry-name')
    await input.waitForDisplayed({ timeout: 5000 })
    assert.equal(await input.getValue(), raw.split('/').at(-1))
    assert.ok(await input.isFocused())
    return input
  }

  async renameDraft(value, { error, repeat = false, button = false } = {}) {
    await this.fill(await this.browser.$('#rename-entry-name'), value)
    if (button) await (await this.browser.$('//*[@role="dialog"]//button[normalize-space(.)="Rename"]')).click()
    else await this.burst('Enter', 1 + Number(repeat), null)
    if (error) {
      const pill = await this.browser.$('[role="dialog"] .pill.error')
      await pill.waitForDisplayed({ timeout: 60_000 })
      assert.match(await pill.getText(), error)
      assert.ok(await (await this.browser.$('#rename-entry-name')).isDisplayed())
    } else await this.idle()
  }

  async cancelRename(escape = false) {
    if (escape) { await this.browser.keys([Key.Escape]); await this.browser.keys([Key.Escape]) }
    else await (await this.browser.$('//*[@role="dialog"]//button[normalize-space(.)="Cancel"]')).click()
    await this.idle()
  }

  async burst(key, count = 3, modifier = 'Control') {
    assert.ok(Number.isSafeInteger(count) && count > 0 && count <= 3)
    const action = this.browser.action('key')
    const value = Key[key] ?? key
    if (modifier) action.down(Key[modifier])
    for (let i = 0; i < count; i++) action.down(value).up(value)
    if (modifier) action.up(Key[modifier])
    try { await action.perform() } finally { await this.browser.releaseActions() }
  }

  async populateClipboard(base, paths, move = false, menu = false, { directories = [] } = {}) {
    ownedPath(this.roots, base)
    assert.ok(paths.length > 0 && new Set(paths).size === paths.length)
    for (const raw of paths) ownedPath(this.roots, raw)
    await this.navigate(base); await this.select(paths[0])
    for (const raw of paths.slice(1)) await this.modifiedSelect(raw, 'Control')
    await this.selection(base, paths, { directories })
    await this.browser.waitUntil(async () => !['Copied', 'Cut'].includes(await this.browser.execute(() =>
      document.querySelector('.toast[role="status"]')?.textContent.trim())), { timeout: 5000 })
    if (menu) await this.menuAction(move ? 'cut' : 'copy', paths[0])
    else await this.chord(move ? 'x' : 'c')
    await this.idle({ toast: move ? 'Cut' : 'Copied', ...(move ? { cutPath: paths[0] } : {}) })
  }

  async paste(dest, firstPath, { menu = false, repeat = false, conflict } = {}) {
    ownedPath(this.roots, dest); ownedPath(this.roots, firstPath)
    await this.navigate(dest); await this.emptySpace(dest)
    if (menu) await this.menuAction('paste')
    else await this.burst('v', repeat ? 3 : 1)
    if (conflict) {
      const dialog = await this.browser.$('.conflict-modal')
      await dialog.waitForDisplayed({ timeout: 60_000 })
      await (await dialog.$(`.//button[normalize-space(.)="${conflict}"]`)).click()
    }
    await this.idle(conflict === 'Cancel' ? {} : { resultPath: firstPath }, 360_000)
  }

  async deleteSelection({ menu = false, raw, cancel = false, escape = false, repeat = false } = {}) {
    if (menu) await this.menuAction('delete-permanent', raw)
    else await this.chord('Delete', 'Shift')
    const dialog = await this.browser.$('[role="dialog"]')
    await dialog.waitForDisplayed({ timeout: 5000 })
    assert.match(await dialog.getText(), /Delete permanently\?/)
    assert.match(await dialog.getText(), /cannot be undone|cannot undo/i)
    if (escape) await this.browser.keys([Key.Escape])
    else if (repeat && !cancel) {
      await (await this.browser.$('[data-confirm-delete="1"]')).click()
      await this.burst('Enter', 2, null)
    } else await (await this.browser.$(cancel ? '[data-cancel-delete="1"]' : '[data-confirm-delete="1"]')).click()
    await this.idle()
  }

  async historyStep(redo, expected = {}, unavailable = false) {
    await this.chord(redo ? 'y' : 'z')
    if (!unavailable) return this.idle({ toast: redo ? 'Redo' : 'Undo', ...expected })
    const text = redo ? 'Nothing to redo' : 'Nothing to undo'
    await this.browser.waitUntil(async () => (await this.browser.execute(() =>
      document.querySelector('.toast[role="status"]')?.textContent.trim() ?? '')).includes(text), { timeout: 5000 })
    await this.browser.waitUntil(async () => !await this.browser.execute(() =>
      document.querySelector('.toast[role="status"]')?.getClientRects().length), { timeout: 10_000 })
    await this.idle()
  }

  async expectedToast(pattern) {
    let text
    await this.browser.waitUntil(async () => {
      text = await this.browser.execute(() => document.querySelector('.toast[role="status"]')?.textContent.trim() ?? '')
      return pattern.test(text)
    }, { timeout: 10_000, timeoutMsg: 'Expected explicit rejection feedback' })
    assert.match(text, pattern)
    await this.browser.waitUntil(async () => !await this.browser.execute(() =>
      document.querySelector('.toast[role="status"]')?.getClientRects().length), { timeout: 10_000 })
    await this.idle()
  }

  async propertiesOpen(base, paths, menu = false, { directories = [] } = {}) {
    ownedPath(this.roots, base)
    assert.ok(paths.length > 0 && paths.length <= 2 && new Set(paths).size === paths.length)
    for (const raw of paths) ownedPath(this.roots, raw)
    await this.navigate(base); await this.select(paths[0])
    for (const raw of paths.slice(1)) await this.modifiedSelect(raw, 'Control')
    await this.selection(base, paths, { directories })
    if (menu) await this.menuAction('properties', paths[0])
    else await this.chord('p')
    await (await this.browser.$('.properties-modal')).waitForDisplayed({ timeout: 5000 })
  }

  async propertiesTab(tab) {
    assert.ok(['Basic', 'Extra', 'Ownership', 'Permissions'].includes(tab))
    await (await this.browser.$(`//div[contains(@class,"properties-modal")]//div[contains(@class,"tabs")]//button[normalize-space(.)="${tab}"]`)).click()
    await this.browser.waitUntil(async () => !(await (await this.browser.$('.properties-modal')).getText()).includes('Loading…'), { timeout: 60_000 })
  }

  async propertiesRows() {
    return this.browser.execute(() => Object.fromEntries([...document.querySelectorAll('.properties-modal .row')]
      // Rows use display: contents; only their rendered children have boxes.
      .filter(row => row.querySelector('.label')?.getClientRects().length && row.querySelector('.value')?.getClientRects().length)
      .map(row => [row.querySelector('.label').textContent.trim(), row.querySelector('.value').textContent.replace(/\s+/g, ' ').trim()])))
  }

  async closeProperties(base) {
    await this.browser.keys([Key.Escape]); await this.idle(); await this.waitPath(base)
  }

  async editingFocus(base, paths) {
    for (const path of paths) ownedPath(this.roots, path)
    await this.waitPath(base)
    await this.browser.waitUntil(async () => await this.browser.execute(paths => {
      const active = document.activeElement, collection = document.querySelector('.rows, .grid')
      return active === collection || (collection?.contains(active) && paths.includes(active.closest('[data-path]')?.dataset.path))
    }, paths), { timeout: 5000, timeoutMsg: 'Closing the dialog must restore focus to the owned collection or its selected trigger' })
  }

  async remove(raw, cancel = false) {
    await this.navigate(raw.slice(0, raw.lastIndexOf('/')))
    await this.select(raw)
    await this.chord('Delete', 'Shift')
    const button = await this.browser.$(cancel ? '[data-cancel-delete="1"]' : '[data-confirm-delete="1"]')
    await button.waitForDisplayed({ timeout: 5000 })
    const dialog = await this.browser.$('[role="dialog"]')
    assert.match(await dialog.getText(), /cannot be undone|cannot undo/i, 'Deletion must explain irreversibility')
    if (cancel && this.cancelDeleteAccessible) await this.cancelDeleteAccessible()
    else await button.click()
    await this.idle()
  }
}

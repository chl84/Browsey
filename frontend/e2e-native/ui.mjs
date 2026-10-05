/* global document, window */
import assert from 'node:assert/strict'
import { Key } from 'webdriverio'
import { child, ownedPath, inside } from './scope.mjs'

// One UI driver and shared cases, not a second set of file-operation semantics.
// Mutations go through real Browsey controls/shortcuts, never direct IPC calls.
export class NativeUi {
  constructor(browser, roots) { this.browser = browser; this.roots = roots }

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

  async listing(raw, view, paths) {
    ownedPath(this.roots, raw)
    assert.ok(['list', 'grid'].includes(view))
    for (const path of paths) ownedPath(this.roots, path)
    const expected = [...paths].sort()
    await this.idle()
    let state
    await this.browser.waitUntil(async () => {
      state = await this.browser.execute(() => {
        const visible = node => node.getClientRects().length > 0
        const collection = [...document.querySelectorAll('.rows, .grid')].find(visible)
        return { current: document.querySelector('main.shell')?.dataset.currentPath,
          view: collection?.classList.contains('grid') ? 'grid' : 'list',
          paths: collection ? [...collection.querySelectorAll('[data-path]')].filter(visible).map(node => node.dataset.path).sort() : [],
          empty: collection?.textContent.includes('No items here.') ?? false }
      })
      // Resolve immediately on an outside observation, then throw outside the
      // WebDriver predicate so a scope failure cannot be swallowed/retried.
      if ([state.current, ...state.paths].filter(Boolean).some(path => !this.roots.some(root => inside(root, path)))) return true
      return state.current === raw && state.view === view && JSON.stringify(state.paths) === JSON.stringify(expected)
        && (expected.length > 0 || state.empty)
    }, { timeout: 60_000, interval: 150, timeoutMsg: 'Owned folder contents/view did not match after navigation' })
    ownedPath(this.roots, state.current)
    for (const path of state.paths) ownedPath(this.roots, path)
    assert.equal(state.current, raw)
    assert.equal(state.view, view)
    assert.deepEqual(state.paths, expected)
    if (!expected.length) assert.ok(state.empty, 'Empty folder must show its empty state')
  }

  async select(raw) {
    ownedPath(this.roots, raw)
    const row = await this.browser.$(`[data-path=${JSON.stringify(raw)}]`)
    await row.waitForDisplayed({ timeout: 60_000 })
    await row.click()
    return row
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
    await this.browser.keys([Key.Backspace])
    const action = this.browser.action('key')
    for (const character of value) {
      if (character === '_') action.down(Key.Shift).down('-').up('-').up(Key.Shift)
      else action.down(character).up(character)
    }
    try { await action.perform() } finally { await this.browser.releaseActions() }
    assert.equal(await input.getValue(), value, 'Native text input differed from the intended value')
    assert.ok(await input.isFocused(), 'Input lost focus before submission')
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

/* global document, window */
import assert from 'node:assert/strict'
import { child, ownedPath } from './scope.mjs'

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

  async idle() {
    await this.browser.waitUntil(async () => {
      return await this.browser.execute(() => document.querySelector('main.shell[aria-busy="false"][data-operation-active="false"]') !== null
        && ![...document.querySelectorAll('[role="dialog"]')].some(node => node.getClientRects().length)
        && ![...document.querySelectorAll('.pill.error')].some(node => node.getClientRects().length))
    }, { timeout: 60_000, interval: 150, timeoutMsg: 'Browsey did not finish without an error/dialog' })
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
    await this.browser.waitUntil(async () => await (await this.browser.$('main.shell')).getAttribute('data-current-path') === raw,
      { timeout: 60_000, timeoutMsg: 'UI did not navigate to the owned fixture folder' })
    await this.idle()
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
    await this.browser.action('key').down(modifier).down(key).up(key).up(modifier).perform()
    await this.browser.releaseActions()
  }

  async refresh() { await this.browser.keys(['F5']); await this.idle() }

  async transfer(src, dest, move = false) {
    await this.navigate(src.slice(0, src.lastIndexOf('/')))
    await this.select(src)
    await this.chord(move ? 'x' : 'c')
    await this.navigate(dest)
    await this.chord('v')
    await this.idle()
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
    await input.setValue(name)
    assert.equal(await input.getValue(), name, 'Native text input differed from the fixture name')
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
    await input.setValue(name)
    assert.equal(await input.getValue(), name, 'Native text input differed from the fixture name')
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

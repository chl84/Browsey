import { expect, test, type Page } from '@playwright/test'

const nativeDrop = (page: Page, paths: string[]) => page.evaluate(paths => {
  window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', {
    detail: { type: 'drop', paths, position: { x: 250, y: 250 } },
  }))
}, paths)

const row = (page: Page, name: string) => page.locator('.rows .row').filter({
  has: page.getByText(name, { exact: true }),
})

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [] }
  })
  await page.goto('/')
  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()
})

test('native drop copies its own file and preserves a previously cut selection', async ({ page }) => {
  const notes = row(page, 'notes')
  await notes.click()
  await page.keyboard.press('Control+x')
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  await expect(row(page, 'report')).toBeVisible()
  await expect(notes).toBeVisible()
  const calls = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: unknown }> }
  }).__BROWSEY_E2E__.calls)
  const transfers = calls.filter(call => call.cmd === 'paste_clipboard_cmd')
  expect(transfers).toHaveLength(1)
  expect(transfers[0].args).toMatchObject({
    dest: '/mock', input: { mode: 'copy', paths: ['/mock/Documents/report.txt'] },
  })
  expect(calls.filter(call => call.cmd === 'clear_system_clipboard')).toHaveLength(0)
})

test('another native drop cannot replace a pending conflict operation', async ({ page }) => {
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  await expect(row(page, 'report')).toBeVisible()
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  const dialog = page.locator('.conflict-modal')
  await expect(dialog).toBeVisible()
  await nativeDrop(page, ['/mock/notes.txt'])
  await expect(dialog.locator('.name')).toHaveText('/mock/Documents/report.txt')
  await dialog.getByRole('button', { name: 'Auto-rename', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(row(page, 'report-1')).toBeVisible()
  await expect(row(page, 'notes-1')).toHaveCount(0)
})

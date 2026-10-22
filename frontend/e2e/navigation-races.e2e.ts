import { expect, test } from '@playwright/test'

type Control = { listingHold: boolean; calls: Array<{ cmd: string; args?: { path?: string } }> }
const control = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], listingHold: false, bookmarks: [
        { label: 'Demo documents', path: '/mock/Documents' },
        { label: 'Demo archive', path: '/mock/Documents/archive' },
      ],
    }
  })
  await page.goto('/')
  await expect(page.locator('main.shell')).toHaveAttribute('data-current-path', '/mock')
})

for (const action of ['bookmark', 'address']) {
  test(`the latest ${action} folder opens after a pending listing finishes`, async ({ page }) => {
    await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.listingHold = true })
    await page.locator('.bookmark[data-drop-path="/mock/Documents"]').click()
    await expect(page.locator('main.shell')).toHaveAttribute('aria-busy', 'true')
    if (action === 'bookmark') {
      await page.locator('.bookmark[data-drop-path="/mock/Documents/archive"]').click()
    } else {
      const input = page.locator('#explorer-path-input')
      await input.focus()
      await input.fill('/mock/Documents/archive')
      await input.press('Enter')
    }
    await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.listingHold = false })
    await expect(page.locator('main.shell')).toHaveAttribute('data-current-path', '/mock/Documents/archive')
    await expect(page.locator('main.shell')).toHaveAttribute('aria-busy', 'false')
    await expect(page.getByText('No items here.', { exact: true })).toBeVisible()
    const calls = (await page.evaluate(control)).calls
    expect(calls.filter(({ cmd, args }) => cmd === 'list_dir' && args?.path === '/mock/Documents/archive')).toHaveLength(1)
    expect(calls.filter(({ cmd }) => cmd === 'open_entry')).toHaveLength(0)
  })
}

test('a newer Recent navigation discards a queued bookmark', async ({ page }) => {
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.listingHold = true })
  await page.locator('.bookmark[data-drop-path="/mock/Documents"]').click()
  await expect(page.locator('main.shell')).toHaveAttribute('aria-busy', 'true')
  await page.locator('.bookmark[data-drop-path="/mock/Documents/archive"]').click()
  await page.getByRole('button', { name: 'Recent', exact: true }).click()
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.listingHold = false })
  await expect(page.locator('main.shell')).toHaveAttribute('data-current-path', 'recent://')
  expect((await page.evaluate(control)).calls.some(({ cmd, args }) => cmd === 'list_dir' && args?.path === '/mock/Documents/archive')).toBe(false)
})

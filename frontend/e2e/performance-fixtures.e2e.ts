import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      thumbnailFixture: true, defaultView: 'list',
      performanceFixture: { entries: 1000, thumbnailDelayMs: 400 }, calls: [],
    }
  })
})

test('large disposable listing stays virtualized through scroll and zoom', async ({ page }) => {
  await page.goto('/')
  const rows = page.locator('.rows')
  await expect(rows).toBeVisible()
  // These are virtualized button collections, not ARIA tables with rows/cells.
  await expect(page.getByRole('group', { name: 'File list' })).toBeVisible()
  await expect(rows).toHaveAttribute('role', 'group')
  await expect(page.getByRole('button', { name: /photo-000/ })).toBeVisible()
  await expect(page.locator('.row')).not.toHaveCount(1002)
  expect(await page.locator('.row').count()).toBeLessThan(200)
  await rows.evaluate(el => { el.scrollTop = el.scrollHeight })
  await expect(page.locator('.row').filter({ hasText: 'photo-999' })).toBeVisible()
  await rows.evaluate(el => {
    el.dispatchEvent(new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY: -120 }))
  })
  await expect(page.locator('.grid')).toBeVisible()
  await expect(page.getByRole('group', { name: 'File grid' })).toBeVisible()
  await expect(page.getByRole('button', { name: /photo-/ }).first()).toBeVisible()
  expect(await page.locator('.card').count()).toBeLessThan(250)
  await expect(page.locator('.card img.icon[src^="data:image"]').first()).toBeVisible()
})

test('controlled thumbnail delay is cancelled on navigation without waiting for old work', async ({ page }) => {
  await page.goto('/')
  await page.locator('.rows').evaluate(el => {
    el.dispatchEvent(new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY: -120 }))
  })
  await expect(page.locator('.grid')).toBeVisible()
  await page.locator('.card[data-path="/mock/Documents"]').dblclick()
  await expect(page.locator('.card[data-path="/mock/Documents/photo-000.jpg"]')).toBeVisible()
  await expect.poll(() => page.evaluate(() => {
    const host = window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string }> } }
    return host.__BROWSEY_E2E__.calls.filter(call => call.cmd === 'cancel_task').length
  })).toBeGreaterThan(0)
  await expect(page.locator('.card[data-path="/mock/Documents/photo-000.jpg"] img.icon'))
    .toHaveAttribute('src', /^data:image/)
})

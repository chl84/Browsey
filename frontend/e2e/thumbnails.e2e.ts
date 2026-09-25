import { expect, test } from '@playwright/test'

type Control = {
  thumbnailFixture: boolean
  thumbnailHold?: boolean
  calls: Array<{ cmd: string; args?: Record<string, unknown> }>
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = { thumbnailFixture: true, calls: [] }
  })
})

test('scrolling away and back reuses loaded thumbnails and skips text files', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/')
  const first = page.locator('.card[data-path="/mock/photo-000.jpg"]')
  await expect(first.locator('img.icon')).toHaveAttribute('src', /^data:image/)
  await first.click()
  await page.locator('.grid').evaluate(el => { el.scrollTop = el.scrollHeight })
  await expect(page.locator('.card[data-path="/mock/photo-099.jpg"] img.icon')).toHaveAttribute('src', /^data:image/)
  await page.locator('.grid').evaluate(el => { el.scrollTop = 0 })
  await expect(first.locator('img.icon')).toHaveAttribute('src', /^data:image/)
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'get_thumbnail' && call.args?.path === '/mock/photo-000.jpg')).toHaveLength(1)
  expect(calls.some(call => call.cmd === 'get_thumbnail' && String(call.args?.path).endsWith('.txt'))).toBe(false)
  expect(errors).toEqual([])
})

test('navigation cancels pending thumbnails and starts the new directory without waiting', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.thumbnailHold = true
  })
  await page.goto('/')
  await expect.poll(() => page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'get_thumbnail').length)).toBe(3)
  await page.locator('.card[data-path="/mock/Documents"]').dblclick()
  const first = page.locator('.card[data-path="/mock/Documents/photo-000.jpg"]')
  await expect(first).toBeVisible()
  await expect.poll(() => page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.some(call => call.cmd === 'get_thumbnail' && call.args?.path === '/mock/Documents/photo-000.jpg'))).toBe(true)
  const cancels = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'cancel_task').length)
  expect(cancels).toBeGreaterThanOrEqual(3)
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.thumbnailHold = false })
  await expect(first.locator('img.icon')).toHaveAttribute('src', /^data:image/)
})

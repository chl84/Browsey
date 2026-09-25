import { expect, test, type Page } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      thumbnailFixture: true, defaultView: 'list', calls: [],
    }
  })
})

const zoom = async (page: Page, deltaY: number) => {
  const consumed = await page.locator('.rows, .grid').evaluate((el, delta) => {
    const event = new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY: delta })
    el.dispatchEvent(event)
    return event.defaultPrevented
  }, deltaY)
  expect(consumed).toBe(true)
  // Separate deliberate notches from the touchpad burst throttle.
  await page.waitForTimeout(100)
}

const expectSize = async (page: Page, size: number) => {
  await expect(page.locator('.card img.icon').first()).toHaveCSS('width', `${size}px`)
  await expect(page.locator('.card img.icon').first()).toHaveCSS('height', `${size}px`)
}

test('Ctrl-wheel traverses list and five grid sizes, clamps and preserves selection', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/')
  const list = page.getByRole('grid', { name: 'File list' })
  await expect(list).toBeVisible()
  await page.locator('.row').filter({ hasText: 'photo-000' }).click()
  await zoom(page, 120)
  await expect(list).toBeVisible()
  for (const size of [64, 96, 128, 160, 192]) {
    await zoom(page, -120)
    await expectSize(page, size)
  }
  await zoom(page, -120)
  await expectSize(page, 192)
  await expect.poll(() => page.evaluate(() => {
    const calls = (window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: Record<string, unknown> }> } }).__BROWSEY_E2E__.calls
    return calls.some(call => call.cmd === 'get_thumbnail' && Number(call.args?.maxDim) >= 192)
  })).toBe(true)
  for (const size of [160, 128, 96, 64]) {
    await zoom(page, 120)
    await expectSize(page, size)
  }
  await zoom(page, 120)
  await expect(list).toBeVisible()
  await expect(page.locator('.row.selected').filter({ hasText: 'photo-000' })).toHaveCount(1)
  expect(errors).toEqual([])
})

test('normal scrolling is unchanged and zoom retains the visible region', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()
  await page.locator('.rows').evaluate(el => { el.scrollTop = 1500 })
  await zoom(page, -120)
  await expectSize(page, 64)
  const centrePath = await page.locator('.grid').evaluate(el => {
    const bounds = el.getBoundingClientRect()
    return [...el.querySelectorAll<HTMLElement>('.card')].find(card => {
      const rect = card.getBoundingClientRect()
      return rect.top <= bounds.top + bounds.height / 2 && rect.bottom >= bounds.top + bounds.height / 2
    })?.dataset.path
  })
  expect(centrePath).toBeTruthy()
  await zoom(page, -120)
  await expectSize(page, 96)
  await expect(page.locator(`.card[data-path="${centrePath}"]`)).toBeInViewport()
  const before = await page.locator('.grid').evaluate(el => el.scrollTop)
  await page.locator('.grid').dispatchEvent('wheel', { deltaY: 120, cancelable: true, bubbles: true })
  await expect.poll(() => page.locator('.grid').evaluate(el => el.scrollTop)).toBeGreaterThan(before)
  await expectSize(page, 96)
})

test('native Ctrl-wheel zooms files, not the page, and compact density keeps zoom', async ({ page }) => {
  await page.goto('/')
  const rows = page.locator('.rows')
  await expect(rows).toBeVisible()
  const ratio = await page.evaluate(() => window.devicePixelRatio)
  await rows.hover()
  await page.keyboard.down('Control')
  await page.mouse.wheel(0, -120)
  await page.keyboard.up('Control')
  await expectSize(page, 64)
  expect(await page.evaluate(() => window.devicePixelRatio)).toBe(ratio)
  await expect(page.locator('.grid-viewport')).toHaveCSS('gap', '8px')

  await page.getByRole('button', { name: 'Main menu' }).click()
  await page.getByRole('menuitem', { name: 'Settings…' }).click()
  const settings = page.locator('.settings-modal')
  await expect(settings).toBeVisible()
  await zoom(page, -120)
  await expectSize(page, 64)
  await settings.getByPlaceholder('Filter settings').fill('density')
  await settings.getByRole('button', { name: 'Cozy' }).click()
  await page.getByRole('option', { name: 'Compact' }).click()
  await page.keyboard.press('Escape')
  await expect(settings).toBeHidden()
  await expect(page.locator('.grid-viewport')).toHaveCSS('gap', '6px')
  await expectSize(page, 64)
  await zoom(page, -120)
  await expectSize(page, 96)
})

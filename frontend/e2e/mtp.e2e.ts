import { expect, test, type Page } from '@playwright/test'

type Control = {
  partitions: Array<{ label: string; path: string; fs: string; removable: boolean }>
  calls: Array<{ cmd: string; args?: Record<string, unknown> }>
  mtpHold?: boolean
  mtpError?: string
}
const phone = { label: 'Android phone', path: 'mtp://Phone_A/', fs: 'mtp', removable: true }
const control = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
const errors = new WeakMap<Page, string[]>()

test.beforeEach(async ({ page }) => {
  const runtimeErrors: string[] = []
  errors.set(page, runtimeErrors)
  page.on('pageerror', (error) => runtimeErrors.push(error.message))
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = { partitions: [], calls: [] }
  })
})
test.afterEach(async ({ page }) => { expect(errors.get(page)).toEqual([]) })

const plugIn = async (page: Page) => {
  await page.evaluate((part) => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.partitions = [part]
    window.dispatchEvent(new Event('browsey-e2e-volumes-changed'))
  }, phone)
}

test('hotplug discovers a phone without mounting it; click mounts once; unplug returns Home', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()
  await plugIn(page)
  const unmounted = page.getByRole('button', { name: 'Android phone (not mounted)', exact: true })
  await expect(unmounted).toBeVisible()
  expect((await page.evaluate(control)).calls.some(({ cmd }) => cmd === 'connect_network_uri')).toBe(false)
  await unmounted.click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: 'Format…' })).toHaveCount(0)
  await expect(page.getByRole('menuitem', { name: 'Mount and open' })).toBeVisible()
  await page.keyboard.press('Escape')
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.mtpHold = true })
  await unmounted.click()
  await unmounted.click()
  expect((await page.evaluate(control)).calls.filter(({ cmd }) => cmd === 'connect_network_uri')).toHaveLength(1)
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.mtpHold = false })
  const mounted = page.getByRole('complementary').getByRole('button', { name: 'Android phone', exact: true })
  await expect(mounted).toHaveCount(1)
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'Phone', exact: true })).toBeVisible()
  await mounted.click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: 'Format…' })).toHaveCount(0)
  await page.getByRole('menuitem', { name: 'Properties', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByText('Phone (MTP)', { exact: true })).toBeVisible()
  await dialog.getByRole('button', { name: 'Ownership', exact: true }).click()
  await expect(dialog.getByText('Ownership is managed by the phone. MTP does not expose Linux ownership settings.')).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Apply ownership' })).toHaveCount(0)
  await page.keyboard.press('Escape')
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.partitions = []
    window.dispatchEvent(new Event('browsey-e2e-volumes-changed'))
  })
  await expect(mounted).toHaveCount(0)
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'mock', exact: true })).toBeVisible()
  await plugIn(page)
  await expect(unmounted).toHaveCount(1)
  expect((await page.evaluate(control)).calls.filter(({ cmd }) => cmd === 'connect_network_uri')).toHaveLength(1)
})

test('locked phone reports useful error and allows retry', async ({ page }) => {
  await page.goto('/')
  await plugIn(page)
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.mtpError = 'Unlock the phone and allow File transfer (MTP).'
  })
  const button = page.getByRole('button', { name: 'Android phone (not mounted)', exact: true })
  await button.click()
  await expect(page.getByRole('status')).toContainText('Phone connection failed: Unlock the phone')
  await expect(button).toBeVisible()
  expect((await page.evaluate(control)).calls.some(({ cmd, args }) => cmd === 'list_dir' && args?.path === '/mock/Phone')).toBe(false)
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.mtpError = undefined })
  await button.click()
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'Phone', exact: true })).toBeVisible()
})

test('unplug during mount does not open a stale phone path', async ({ page }) => {
  await page.goto('/')
  await plugIn(page)
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.mtpHold = true })
  await page.getByRole('button', { name: 'Android phone (not mounted)', exact: true }).click()
  await page.evaluate(() => {
    const state = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    state.partitions = []
    state.mtpHold = false
    window.dispatchEvent(new Event('browsey-e2e-volumes-changed'))
  })
  await expect(page.getByRole('status')).toContainText('Phone disconnected or unavailable')
  await expect(page.getByRole('button', { name: 'Android phone (not mounted)', exact: true })).toHaveCount(0)
  expect((await page.evaluate(control)).calls.some(({ cmd, args }) => cmd === 'list_dir' && args?.path === '/mock/Phone')).toBe(false)
})

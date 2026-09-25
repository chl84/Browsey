import { expect, test, type Page } from '@playwright/test'

type Control = { partitions?: Array<{ label: string; path: string; fs: string; removable: boolean }>; failCommands?: string[]; calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
const control = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
const usb = { label: 'USB', path: '/mock/USB', fs: 'exfat', removable: true }
const runtimeErrors = new WeakMap<Page, string[]>()

test.beforeEach(async ({ page }) => {
  const errors: string[] = []
  runtimeErrors.set(page, errors)
  page.on('pageerror', (error) => errors.push(error.message))
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = { calls: [] }
  })
})

test.afterEach(async ({ page }) => {
  expect(runtimeErrors.get(page)).toEqual([])
})

const openFileMenu = async (page: Page) => {
  await page.locator('.row', { has: page.locator('.name', { hasText: 'notes' }) }).click({ button: 'right' })
}

test('compression error remains visible and can be retried', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['compress_entries'] })
  await openFileMenu(page)
  await page.getByRole('menuitem', { name: 'Compress…', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Compress', exact: true })
  await dialog.getByRole('button', { name: 'Create', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Simulated compress_entries failure')
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = [] })
  await dialog.getByRole('button', { name: 'Create', exact: true }).click()
  await expect(dialog).not.toBeVisible()
})

test('Open with never launches an application hidden by the filter', async ({ page }) => {
  await page.goto('/')
  await openFileMenu(page)
  await page.getByRole('menuitem', { name: 'Open with…', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Open with…' })
  const filter = dialog.getByPlaceholder('Filter apps')
  await filter.fill('no-such-app')
  await expect(dialog.getByRole('button', { name: 'Open', exact: true })).toBeDisabled()
  await filter.press('Enter')
  await expect(dialog).toBeVisible()
  await expect(dialog.getByText('No applications match your filter.')).toBeVisible()
  await filter.fill('Beta')
  await filter.press('Enter')
  await expect(dialog).not.toBeVisible()
  const calls = await page.evaluate(control)
  expect(calls.calls.filter((call) => call.cmd === 'open_with')).toMatchObject([{ args: { choice: { appId: 'beta' } } }])
})

test('USB format failure stays visible, is copyable, and requires reinspection', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.goto('/')
  await page.evaluate((part) => {
    const state = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    state.partitions = [part]
    state.failCommands = ['format_removable_partition']
    window.dispatchEvent(new Event('browsey-e2e-volumes-changed'))
  }, usb)
  await page.getByRole('button', { name: 'USB', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Format…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Format USB drive?' })
  await dialog.getByRole('button', { name: 'Format and erase' }).click()
  await expect(dialog.getByRole('alert')).toContainText('Simulated format_removable_partition failure')
  await expect(dialog.getByRole('button', { name: 'Format and erase' })).toBeDisabled()
  await dialog.getByRole('button', { name: 'Copy error details' }).click()
  await expect(dialog.getByRole('status')).toHaveText('Error details copied.')
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain('Simulated format_removable_partition failure')
  await dialog.getByRole('button', { name: 'Inspect again' }).click()
  await expect(dialog.getByRole('button', { name: 'Format and erase' })).toBeEnabled()
})

test('hotplug exposes an unmounted USB and its keyboard menu mounts it', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()
  await page.evaluate((part) => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.partitions = [part]
    window.dispatchEvent(new Event('browsey-e2e-volumes-changed'))
  }, { ...usb, path: 'usb-volume:///dev/sdz1' })
  const button = page.getByRole('button', { name: 'USB (not mounted)', exact: true })
  await expect(button).toBeVisible({ timeout: 3000 })
  await button.focus()
  await page.keyboard.press('Shift+F10')
  await expect(page.getByRole('menuitem', { name: 'Mount and open' })).toBeFocused()
  await page.keyboard.press('ArrowDown')
  await expect(page.getByRole('menuitem', { name: 'Format…' })).toBeFocused()
  await page.keyboard.press('Home')
  await expect(page.getByRole('menuitem', { name: 'Mount and open' })).toBeFocused()
  await page.keyboard.press('End')
  await expect(page.getByRole('menuitem', { name: 'Format…' })).toBeFocused()
  await page.keyboard.press('ArrowUp')
  await expect(page.getByRole('menuitem', { name: 'Mount and open' })).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(button).toBeFocused()
  await page.keyboard.press('Shift+F10')
  await page.keyboard.press('Enter')
  await expect(page.getByRole('complementary').getByRole('button', { name: 'USB', exact: true })).toBeVisible()
  const state = await page.evaluate(control)
  expect(state.calls.some((call) => call.cmd === 'list_dir' && call.args?.path === '/mock/USB')).toBe(true)
})

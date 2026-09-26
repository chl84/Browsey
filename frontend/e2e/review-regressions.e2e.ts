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

test('Open with only saves a checked default on Open and keeps save errors retryable', async ({ page }) => {
  await page.goto('/')
  await openFileMenu(page)
  await page.getByRole('menuitem', { name: 'Open with…', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Open with…' })
  const save = dialog.getByRole('checkbox', { name: 'Set as default', exact: true })
  const open = dialog.getByRole('button', { name: 'Open', exact: true })
  await expect(save).not.toBeChecked()
  await expect(save).toBeDisabled()
  await dialog.getByPlaceholder('Filter apps').fill('Beta')
  await expect(dialog.getByText('When checked, Open also sets')).toContainText('text/plain')
  await expect(save).toBeEnabled()
  await save.check()
  expect((await page.evaluate(control)).calls.some((call) => call.cmd === 'set_default_app')).toBe(false)
  await dialog.getByPlaceholder('Filter apps').fill('no-such-app')
  await expect(save).toBeDisabled()
  await expect(save).not.toBeChecked()
  await dialog.getByPlaceholder('Filter apps').fill('Beta')
  await save.check()
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['set_default_app'] })
  await open.click()
  await expect(dialog.getByRole('alert')).toContainText('Simulated set_default_app failure')
  expect((await page.evaluate(control)).calls.some((call) => call.cmd === 'open_with')).toBe(false)
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = [] })
  await expect(save).toBeChecked()
  await open.click()
  await expect(dialog).not.toBeVisible()
  const { calls } = await page.evaluate(control)
  expect(calls.filter((call) => call.cmd === 'open_with')).toMatchObject([{ args: { choice: { appId: 'beta' } } }])
  expect(calls.filter((call) => call.cmd === 'set_default_app')).toHaveLength(2)
  expect(calls.filter((call) => call.cmd === 'set_default_app')[1]).toMatchObject({ args: { appId: 'beta', contentType: 'text/plain' } })
})

test('Open with checkbox is left of Cancel, resets after cancellation, and supports keyboard Open', async ({ page }) => {
  await page.goto('/')
  const showDialog = async () => {
    await openFileMenu(page)
    await page.getByRole('menuitem', { name: 'Open with…', exact: true }).click()
  }
  await showDialog()
  const dialog = page.getByRole('dialog', { name: 'Open with…' })
  const checkbox = dialog.getByRole('checkbox', { name: 'Set as default' })
  const filter = dialog.getByPlaceholder('Filter apps')
  await filter.fill('Beta')
  const checkboxBox = await checkbox.boundingBox()
  const cancelBox = await dialog.getByRole('button', { name: 'Cancel', exact: true }).boundingBox()
  expect(checkboxBox!.x).toBeLessThan(cancelBox!.x)
  await checkbox.check()
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  expect((await page.evaluate(control)).calls.some((call) => ['open_with', 'set_default_app'].includes(call.cmd))).toBe(false)
  await showDialog()
  await filter.fill('Beta')
  await expect(checkbox).not.toBeChecked()
  await checkbox.check()
  await filter.press('Enter')
  await expect(dialog).not.toBeVisible()
  const { calls } = await page.evaluate(control)
  expect(calls.filter((call) => ['open_with', 'set_default_app'].includes(call.cmd)).map((call) => call.cmd)).toEqual(['set_default_app', 'open_with'])
})

test('double-click reports a failed default-app launch and permits retry', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['open_entry'] })
  const file = page.locator('.row', { has: page.locator('.name', { hasText: 'notes' }) })
  await file.dblclick()
  await expect(page.getByText('Simulated open_entry failure', { exact: false }).first()).toBeVisible()
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = [] })
  await file.dblclick()
  const { calls } = await page.evaluate(control)
  expect(calls.filter((call) => call.cmd === 'open_entry')).toMatchObject([
    { args: { path: '/mock/notes.txt' } },
    { args: { path: '/mock/notes.txt' } },
  ])
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
  await expect(page.getByRole('menuitem', { name: 'Properties', exact: true })).toBeFocused()
  await page.keyboard.press('ArrowUp')
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

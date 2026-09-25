import { expect, test } from '@playwright/test'

type Control = {
  calls: Array<{ cmd: string; args?: Record<string, unknown> }>
  partitions: Array<{ label: string; path: string; fs: string; removable: boolean }>
}
const getControl = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = {
      partitions: [{ label: 'USB', path: '/mock/USB', fs: 'btrfs', removable: true }], calls: [],
    }
  })
})

test('USB context menu reuses Properties with ownership and permissions', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.goto('/')
  await page.getByRole('button', { name: 'USB', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Properties', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByText('btrfs', { exact: true })).toBeVisible()
  await expect(dialog.getByText('/mock/USB', { exact: true })).toBeVisible()
  await expect(dialog.getByLabel('Hidden attribute')).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: 'Extra', exact: true })).toHaveCount(0)
  await dialog.getByRole('button', { name: 'Copy drive path' }).click()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('/mock/USB')
  await dialog.getByRole('button', { name: 'Ownership', exact: true }).click()
  await expect(dialog.getByRole('button', { name: 'Apply ownership' })).toBeEnabled()
  await expect(dialog.getByRole('button', { name: 'chris', exact: true })).toBeVisible()
  await dialog.getByRole('button', { name: 'Permissions', exact: true }).click()
  await expect(dialog.getByLabel('Owner write permission')).toBeChecked()
  await expect(dialog.getByLabel('Other users write permission')).not.toBeChecked()
  await expect(dialog.getByText('Changes apply only to the drive’s root folder, not its contents.')).toBeVisible()
  const state = await page.evaluate(getControl)
  expect(state.calls.filter(({ cmd }) => cmd === 'get_permissions')).toEqual([
    { cmd: 'get_permissions', args: { path: '/mock/USB' } },
  ])
  expect(state.calls.some(({ cmd }) => ['set_permissions', 'set_ownership', 'set_hidden', 'format_removable_partition', 'mount_usb_volume'].includes(cmd))).toBe(false)

  await page.keyboard.press('Escape')
  await expect(dialog).not.toBeVisible()
  await page.locator('.row', { has: page.locator('.name', { hasText: 'notes' }) }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: /^Properties/ }).click()
  await expect(dialog.getByLabel('Hidden attribute')).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Extra', exact: true })).toBeVisible()
})

test('unmounted USB properties show device details without mounting it', async ({ page }) => {
  await page.addInitScript(() => {
    const state = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    state.partitions[0].path = 'usb-volume:///dev/sdz1'
  })
  await page.goto('/')
  const usb = page.getByRole('button', { name: 'USB (not mounted)', exact: true })
  await usb.focus()
  await page.keyboard.press('Shift+F10')
  await page.keyboard.press('End')
  await expect(page.getByRole('menuitem', { name: 'Properties', exact: true })).toBeFocused()
  await page.keyboard.press('Enter')
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByText('/dev/sdz1', { exact: true })).toBeVisible()
  await expect(dialog.getByText('Not mounted', { exact: true })).toBeVisible()
  await dialog.getByRole('button', { name: 'Ownership', exact: true }).click()
  await expect(dialog.getByText('Mount the drive to view ownership.')).toBeVisible()
  await dialog.getByRole('button', { name: 'Permissions', exact: true }).click()
  await expect(dialog.getByText('Mount the drive to view permissions.')).toBeVisible()
  const state = await page.evaluate(getControl)
  expect(state.calls.some(({ cmd }) => ['mount_usb_volume', 'get_permissions', 'set_permissions', 'set_ownership', 'set_hidden'].includes(cmd))).toBe(false)
})

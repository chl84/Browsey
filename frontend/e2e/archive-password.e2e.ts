import { expect, test, type Page } from '@playwright/test'

type Control = { calls: Array<{ cmd: string; args?: Record<string, unknown> }>; archivePassword?: string }
const errors = new WeakMap<Page, string[]>()
test.beforeEach(async ({ page }) => {
  const captured: string[] = []
  errors.set(page, captured)
  page.on('pageerror', (error) => captured.push(error.message))
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = { calls: [], archivePassword: 'test secret' }
  })
  await page.goto('/')
})
test.afterEach(async ({ page }) => { expect(errors.get(page)).toEqual([]) })
const menu = async (page: Page, name: string) => {
  await page.locator('.row', { has: page.locator('.name', { hasText: 'notes' }) }).click({ button: 'right' })
  await page.getByRole('menuitem', { name, exact: true }).click()
}

test('password creation validates confirmation, masks input and resets on reopen', async ({ page }) => {
  await menu(page, 'Compress…')
  const dialog = page.getByRole('dialog', { name: 'Compress', exact: true })
  const protect = dialog.getByRole('checkbox', { name: 'Protect with password' })
  await expect(protect).not.toBeChecked()
  await protect.check()
  await expect(dialog.getByText('File names remain visible.', { exact: false })).toBeVisible()
  await dialog.getByRole('button', { name: 'Create', exact: true }).click()
  await expect(dialog.getByRole('alert')).toHaveText('Enter a password.')
  const password = dialog.getByLabel('Password', { exact: true })
  const confirmation = dialog.getByLabel('Confirm password', { exact: true })
  await expect(password).toHaveAttribute('type', 'password')
  await password.fill('test secret')
  await confirmation.fill('different')
  await confirmation.press('Enter')
  await expect(dialog.getByRole('alert')).toHaveText('Passwords do not match.')
  await confirmation.fill('test secret')
  await confirmation.press('Enter')
  await expect(dialog).not.toBeVisible()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
  expect(calls.filter((call) => call.cmd === 'compress_entries')).toMatchObject([{ args: { password: 'test secret' } }])
  await menu(page, 'Compress…')
  await expect(protect).not.toBeChecked()
  await protect.check()
  await expect(password).toHaveValue('')
  await expect(confirmation).toHaveValue('')
})

test('extraction retries a wrong password, clears input and supports cancellation', async ({ page }) => {
  await menu(page, 'Extract')
  const dialog = page.getByRole('dialog', { name: 'Archive password', exact: true })
  const password = dialog.getByLabel('Password', { exact: true })
  await expect(password).toBeFocused()
  await expect(password).toHaveAttribute('type', 'password')
  await password.fill('wrong')
  await password.press('Enter')
  await expect(dialog.getByRole('alert')).toHaveText('Incorrect archive password')
  await expect(password).toHaveValue('')
  await password.fill('test secret')
  await password.press('Enter')
  await expect(dialog).not.toBeVisible()
  await menu(page, 'Extract')
  await expect(password).toHaveValue('')
  await dialog.getByRole('button', { name: 'Cancel extraction' }).click()
  await expect(dialog).not.toBeVisible()
})

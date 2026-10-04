import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [] }
  })
  await page.goto('/')
  await expect(page.locator('.row .name', { hasText: 'notes' })).toBeVisible()
})

test('typing filters files and Backspace returns to breadcrumbs without navigating', async ({ page }) => {
  await page.locator('.row .name', { hasText: 'notes' }).click()
  await page.keyboard.press('n')
  await expect(page.getByLabel('Path', { exact: true })).toHaveValue('n')
  await page.getByLabel('Path', { exact: true }).fill('notes')
  await expect(page.locator('.row')).toHaveCount(1)
  for (const draft of ['note', 'not', 'no', 'n']) {
    await page.locator('.row .name', { hasText: 'notes' }).click()
    await page.keyboard.press('Backspace')
    await expect(page.getByLabel('Path', { exact: true })).toHaveValue(draft)
  }
  await page.locator('.row .name', { hasText: 'notes' }).click()
  await page.keyboard.press('Backspace')
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'mock', exact: true })).toBeVisible()
  await expect(page.locator('.row')).toHaveCount(2)
})

test('rename shortcut uses the selected entry and Escape preserves it', async ({ page }) => {
  await page.locator('.row .name', { hasText: 'notes' }).click()
  await page.keyboard.press('Control+R')
  const dialog = page.getByRole('dialog', { name: 'Rename', exact: true })
  await expect(dialog).toBeVisible()
  await expect(dialog.locator('#rename-entry-name')).toHaveValue('notes.txt')
  await expect(dialog.locator('#rename-entry-name')).toBeFocused()
  // Shared Escape behavior first blurs a text input, then closes the dialog.
  await page.keyboard.press('Escape')
  await expect(dialog.locator('#rename-entry-name')).not.toBeFocused()
  await expect(dialog).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)
  await expect(page.locator('.row.selected .name')).toHaveText('notes')
})

test('properties shortcut resolves a new selection after closing the previous modal', async ({ page }) => {
  for (const [row, name] of [['notes', 'notes.txt'], ['Documents', 'Documents']]) {
    await page.locator('.row .name', { hasText: row }).click()
    await page.keyboard.press('Control+P')
    const dialog = page.getByRole('dialog', { name: /^Properties\b/ })
    await expect(dialog).toBeVisible()
    await expect(dialog.getByText(name, { exact: true })).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(dialog).toHaveCount(0)
  }
})

test('console and select-all use the new directory rather than captured initial state', async ({ page }) => {
  const directory = page.locator('.row', { has: page.locator('.name', { hasText: 'Documents' }) })
  await directory.click()
  await directory.press('Enter')
  await expect(page.locator('.row .name', { hasText: 'report' })).toBeVisible()
  await page.keyboard.press('Control+T')
  await expect.poll(async () => page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: { path?: string } }> } }).__BROWSEY_E2E__
    return control.calls.filter(call => call.cmd === 'open_console').at(-1)?.args?.path
  })).toBe('/mock/Documents')
  await page.keyboard.press('Control+A')
  await expect(page.locator('.row.selected')).toHaveCount(await page.locator('.row').count())
  await expect(page.locator('.row.selected .name', { hasText: 'report' })).toBeVisible()
  await expect(page.locator('.row.selected .name', { hasText: 'notes' })).toHaveCount(0)
})

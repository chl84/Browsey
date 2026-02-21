import { expect, test } from '@playwright/test'

test('opens a directory from list view with keyboard open', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()

  const rootDirRow = page.locator('.row', {
    has: page.locator('.name', { hasText: 'Documents' }),
  })
  await expect(rootDirRow).toBeVisible()

  await rootDirRow.click()
  await rootDirRow.press('Enter')

  await expect(page.locator('.row .name', { hasText: 'report' })).toBeVisible()
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'Documents' })).toBeVisible()
})

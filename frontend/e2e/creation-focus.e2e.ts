import { expect, test } from '@playwright/test'

for (const view of ['list', 'grid'] as const) {
  for (const submit of ['Enter', 'Create'] as const) {
    test(`${view}: create folder with ${submit}, then Enter opens it`, async ({ page }) => {
      await page.addInitScript(({ view }) => {
        ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
          calls: [], defaultView: view,
        }
      }, { view })
      await page.goto('/')
      const collection = page.getByRole('group', { name: view === 'list' ? 'File list' : 'File grid', exact: true })
      await expect(collection).toBeVisible()
      // Keep focus on another control so modal restoration alone cannot fix this.
      const address = page.locator('input').first()
      await address.focus()
      await collection.click({ button: 'right', position: { x: 3, y: 150 } })
      await page.getByRole('menuitem', { name: 'New Folder…', exact: true }).click()
      const dialog = page.getByRole('dialog', { name: 'New folder', exact: true })
      const input = dialog.locator('input')
      const name = `Created ${view} ${submit}`
      await input.fill(name)
      if (submit === 'Enter') await input.press('Enter')
      else await dialog.getByRole('button', { name: 'Create', exact: true }).click()
      await expect(dialog).toHaveCount(0)
      await expect(collection).toBeFocused()
      await expect(collection.locator('[data-path].selected')).toHaveAttribute('data-path', `/mock/${name}`)
      await page.keyboard.press('Enter')
      await expect(address).toHaveValue(`/mock/${name}`)
      await expect(collection.getByText('No items here.', { exact: true })).toBeVisible()
    })
  }
}

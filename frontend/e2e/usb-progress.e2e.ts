import { expect, test } from '@playwright/test'

type UsbControl = {
  formatHold: boolean
  formatProgress?: { phase: string; percent: number | null }
  formatError?: { code: string; message: string }
  calls: Array<{ cmd: string }>
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      partitions: [{ label: 'USB', path: '/mock/USB', fs: 'exfat', removable: true }],
      formatHold: true, calls: [],
    }
  })
})

test('formatting has honest progress, prevents repeat erase, and completes only after the reply', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'USB', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Format…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Format USB drive?' })
  await dialog.getByLabel('Filesystem').selectOption('ext4')
  await dialog.getByRole('button', { name: 'Format and erase' }).click()
  const progress = dialog.getByRole('progressbar', { name: 'Formatting progress' })
  await expect(progress).toBeVisible()
  await expect(progress).not.toHaveAttribute('aria-valuenow')
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled()
  await expect(dialog.getByRole('button', { name: 'Working...' })).toBeDisabled()
  await page.keyboard.press('Escape')
  await expect(dialog).toBeVisible()
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.formatProgress = { phase: 'Creating filesystem', percent: 42 }
  })
  await expect(progress).toHaveAttribute('aria-valuenow', '42')
  await expect(dialog.getByRole('status')).toHaveText('Creating filesystem')
  await expect(dialog.getByText('42% of current step')).toBeVisible()
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.formatProgress = { phase: 'Mounting USB drive', percent: null }
  })
  await expect(dialog.getByRole('status')).toHaveText('Mounting USB drive')
  await expect(progress).not.toHaveAttribute('aria-valuenow')
  await expect(page.getByText('USB drive ready', { exact: true })).not.toBeVisible()
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.formatHold = false })
  await expect(page.getByRole('dialog', { name: 'USB drive ready' })).toBeVisible()
  await expect(page.getByRole('progressbar', { name: 'Formatting progress' })).not.toBeVisible()
  const count = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.calls.filter((call) => call.cmd === 'format_removable_partition').length)
  expect(count).toBe(1)
})

test('lost reply is reported as unknown and does not offer immediate reformatting', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'USB', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Format…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Format USB drive?' })
  await dialog.getByRole('button', { name: 'Format and erase' }).click()
  await expect(dialog.getByRole('progressbar')).toBeVisible()
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__
    control.formatError = { code: 'format_status_unknown', message: 'The system may still be working. Do not unplug the drive.' }
    control.formatHold = false
  })
  await expect(dialog.getByRole('alert')).toContainText('Formatting status unknown:')
  await expect(dialog.getByRole('alert')).not.toContainText('Format failed:')
  await expect(dialog.getByRole('button', { name: 'Format and erase' })).toBeDisabled()
  await expect(dialog.getByRole('progressbar')).not.toBeVisible()
})

import { expect, test } from '@playwright/test'

type UsbControl = {
  formatHold: boolean
  ntfsFormatAvailable?: boolean
  formatProgress?: { phase: string; percent: number | null }
  formatError?: { code: string; message: string }
  calls: Array<{ cmd: string; args?: Record<string, unknown> }>
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

test('NTFS supports a long label and switching to FAT requires correcting it without truncation', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'USB', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Format…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Format USB drive?' })
  const filesystem = dialog.getByRole('combobox', { name: 'Filesystem', exact: true })
  await filesystem.selectOption('ntfs')
  const label = dialog.getByLabel('Volume name')
  await expect(label).toHaveAttribute('maxlength', '128')
  await label.fill('Windows_backup_2026')
  await filesystem.selectOption('fat32')
  await expect(label).toHaveValue('Windows_backup_2026')
  await expect(dialog.getByRole('alert')).toContainText('up to 11 ASCII')
  await expect(dialog.getByRole('button', { name: 'Format and erase' })).toBeDisabled()
  await filesystem.selectOption('ntfs')
  await expect(dialog.getByRole('button', { name: 'Format and erase' })).toBeEnabled()
  await dialog.getByRole('button', { name: 'Format and erase' }).click()
  await expect(dialog.getByRole('progressbar')).toBeVisible()
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.formatHold = false })
  await expect(page.getByRole('dialog', { name: 'USB drive ready' })).toContainText('Windows_backup_2026 is formatted as NTFS')
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'format_removable_partition'))
  expect(calls).toHaveLength(1)
  expect(calls[0].args).toMatchObject({ filesystem: 'ntfs', label: 'Windows_backup_2026' })
})

test('NTFS remains visible but disabled when its utility is missing', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.ntfsFormatAvailable = false })
  await page.getByRole('button', { name: 'USB', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Format…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Format USB drive?' })
  await expect(dialog.getByRole('combobox', { name: 'Filesystem', exact: true })
    .locator('option[value="ntfs"]')).toHaveAttribute('disabled', '')
  await expect(dialog.getByRole('combobox', { name: 'Filesystem', exact: true })).toHaveValue('exfat')
  await expect(dialog).toContainText('NTFS requires mkntfs')
  await expect(dialog.getByRole('button', { name: 'Format and erase' })).toBeEnabled()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: UsbControl }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'format_removable_partition'))
  expect(calls).toHaveLength(0)
})

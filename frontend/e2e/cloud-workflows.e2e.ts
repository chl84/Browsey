import { expect, test } from '@playwright/test'

test('working copies are accessible offline; upload is explicit and reports a changed original', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], cloudUploadChanged: true,
      cloudCopies: [{ id: '1-2-3', sourcePath: 'rclone://test/report.txt', localPath: '/mock/private/report.txt',
        originalSize: 8, originalModified: null, originalHash: 'hash', createdAt: 1,
        dirty: true, uploadedPath: null }],
    }
  })
  await page.goto('/')
  await page.keyboard.press('Control+s')
  await page.getByPlaceholder('Filter settings').fill('cloud')
  await page.getByRole('button', { name: 'Working copies…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Cloud working copies', exact: true })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByText('Locally modified', { exact: false })).toBeVisible()
  const calls = () => page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
  }).__BROWSEY_E2E__.calls)
  expect((await calls()).filter(({ cmd }) => cmd === 'upload_cloud_working_copy')).toHaveLength(0)
  await dialog.getByRole('button', { name: 'Upload changes as new file' }).click()
  await expect(dialog.getByRole('status')).toContainText('The cloud original changed.')
  await expect(dialog.getByRole('status')).toContainText('The original and working copy were kept.')
  expect((await calls()).filter(({ cmd }) => cmd === 'upload_cloud_working_copy')).toMatchObject([{ args: { id: '1-2-3' } }])
  await dialog.getByRole('button', { name: 'Show storage folder' }).click()
  expect((await calls()).filter(({ cmd }) => cmd === 'open_entry')).toMatchObject([{ args: { path: '/mock/browsey/cloud-workspaces' } }])
})

test('cloud external export prepares local paths and requests copy-only even with Shift', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], startupPath: 'rclone://test/disposable', cloudFixture: true,
    }
  })
  await page.goto('/')
  const row = page.locator('.row', { has: page.locator('.name', { hasText: 'report' }) })
  await expect(row).toBeVisible()
  await row.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Prepare external copy…', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Copy cloud files to another app', exact: true })
  const dragButton = dialog.getByRole('button', { name: /Drag .* prepared/ })
  await expect(dragButton).toBeDisabled()
  await dialog.getByRole('button', { name: 'Prepare copies', exact: true }).click()
  await expect(dragButton).toBeEnabled()
  await expect(dragButton).toHaveAttribute('draggable', 'true')
  const payload = await dragButton.evaluate((button) => {
    const transfer = new DataTransfer()
    // Synthetic browser events do not have a native writable drag data store.
    // Capture the requested action policy; real GTK receiver acceptance is
    // separate, not claimed by this frontend routing test.
    Object.defineProperty(transfer, 'effectAllowed', { value: 'none', writable: true })
    button.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer: transfer, shiftKey: true }))
    const payload = { effectAllowed: transfer.effectAllowed,
      paths: transfer.getData('application/x-browsey-paths'), uris: transfer.getData('text/uri-list') }
    button.dispatchEvent(new DragEvent('dragend', { bubbles: true }))
    return payload
  })
  expect(payload.effectAllowed).toBe('copy')
  expect(JSON.parse(payload.paths)).toEqual(['/mock/browsey/cloud-workspaces/export/inputs/report.txt'])
  expect(payload.uris).toContain('file:///mock/')
  expect(payload.uris).not.toContain('rclone://')
})

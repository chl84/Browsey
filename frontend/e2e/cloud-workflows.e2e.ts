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
  await dialog.getByRole('button', { name: 'Save as new file' }).click()
  await expect(dialog.getByText(/The cloud original changed\. Saved as a new file:/)).toBeVisible()
  await expect(dialog.getByText(/The original and working copy were kept\./)).toBeVisible()
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

test('cloud saving stays visible until confirmation and conflicts preserve recovery actions', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [],
      cloudStatuses: [{ id: '1-2-3', name: 'same.txt', sourcePath: 'rclone://test/same.txt', status: 'uploading', bytes: 8, total: 8, sequence: 1 }],
      cloudCopies: [{ id: '1-2-3', sourcePath: 'rclone://test/same.txt', localPath: '/mock/private/same.txt', originalSize: 8, originalModified: null, originalHash: 'hash', createdAt: 1, dirty: true, uploadedPath: null, autoSave: true, saveStatus: 'uploading' }],
    }
  })
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Cloud saves: Saving 1…' })).toBeVisible()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('browsey-e2e-cloud-writeback', { detail: { id: '1-2-3', name: 'same.txt', sourcePath: 'rclone://test/same.txt', status: 'conflict', message: 'Original changed; local edits kept', bytes: 0, total: 0, sequence: 2 } })))
  await page.getByRole('button', { name: 'Cloud saves: 1 need attention' }).click()
  const dialog = page.getByRole('dialog', { name: 'Cloud working copies', exact: true })
  await expect(dialog).toBeVisible()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('browsey-e2e-cloud-writeback', { detail: { id: '1-2-3', name: 'same.txt', sourcePath: 'rclone://test/same.txt', status: 'conflict', message: 'Original changed; local edits kept', bytes: 0, total: 0, sequence: 3 } })))
  await expect(dialog.getByText('Conflict — local edits kept')).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Save as new file' })).toBeEnabled()
  await expect(dialog.getByRole('button', { name: 'Open copy', exact: true })).toBeEnabled()
  await dialog.getByRole('button', { name: 'Close', exact: true }).click()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('browsey-e2e-cloud-writeback', { detail: { id: '1-2-3', name: 'same.txt', sourcePath: 'rclone://test/same.txt', status: 'saved', bytes: 0, total: 0, sequence: 4 } })))
  await expect(page.getByRole('button', { name: 'Cloud saves: Saved', exact: true })).toBeVisible()
  // A manual edit made after a confirmed save must remain actionable when the
  // dialog is reopened; old live Saved events cannot override a fresh listing.
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: { cloudCopies: Array<{ dirty: boolean; autoSave: boolean; saveStatus: string }> } }).__BROWSEY_E2E__
    Object.assign(control.cloudCopies[0], { dirty: true, autoSave: false, saveStatus: 'manual' })
  })
  await page.getByRole('button', { name: 'Cloud saves: Saved', exact: true }).click()
  await expect(dialog.getByText('Manual saving', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Save as new file' })).toBeEnabled()

})

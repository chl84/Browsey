import { expect, test, type Locator } from '@playwright/test'

const chooseAction = async (dialog: Locator, trigger: string, option: string) => {
  await dialog.getByRole('combobox', { name: trigger, exact: true }).click()
  await dialog.getByRole('option', { name: option, exact: true }).click()
}
const expectDisabledAction = async (dialog: Locator, trigger: string, option: string) => {
  const combo = dialog.getByRole('combobox', { name: trigger, exact: true })
  await combo.click()
  await expect(dialog.getByRole('option', { name: option, exact: true })).toHaveAttribute('aria-disabled', 'true')
  await combo.press('Escape')
  await expect(dialog).toBeVisible()
}

test('working-copy action menus stay visible at the end of a scrolled list and support repeated keyboard actions', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 720 })
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], cloudCopies: Array.from({ length: 12 }, (_, index) => ({
        id: `abc-${index}`, sourcePath: `rclone://test/copy-${index}.txt`, localPath: `/mock/private/copy-${index}.txt`,
        originalSize: 8, originalModified: null, originalHash: 'hash', createdAt: 1,
        dirty: false, uploadedPath: null, saveStatus: 'saved', storageBytes: 100,
      })),
    }
  })
  await page.goto('/')
  await page.keyboard.press('Control+s')
  await page.getByPlaceholder('Filter settings').fill('cloud')
  await page.getByRole('button', { name: 'Working copies…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Cloud working copies', exact: true })
  const actions = dialog.getByRole('combobox', { name: 'Actions for rclone://test/copy-11.txt', exact: true })
  await actions.scrollIntoViewIfNeeded()
  await actions.click()
  const option = dialog.getByRole('option', { name: 'Show local folder', exact: true })
  await expect(option).toBeVisible()
  expect(await option.evaluate(element => {
    const rect = element.getBoundingClientRect()
    return rect.top >= 0 && rect.bottom <= innerHeight
      && element.contains(document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2))
  })).toBe(true)
  await actions.press('Escape')
  await expect(dialog).toBeVisible()
  await expect(actions).toBeFocused()
  for (let count = 0; count < 2; count++) {
    await actions.press('ArrowDown')
    await expect(dialog.locator('.combo-list .active')).toHaveText('Open copy')
    await actions.press('Enter')
    await expect(actions).toHaveText('Actions… ▾')
    await expect(actions).toHaveAttribute('aria-expanded', 'false')
  }
  await actions.press('ArrowDown')
  await actions.press('ArrowDown')
  await actions.press('ArrowDown')
  // Save/upload are disabled for unchanged files and skipped by arrow keys.
  await expect(dialog.locator('.combo-list .active')).toHaveText('Resume automatic saving')
  await actions.press('Tab')
  await expect(actions).toHaveAttribute('aria-expanded', 'false')
  await expect(dialog).toBeVisible()
  const calls = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
  }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'open_entry')).toMatchObject([
    { args: { path: '/mock/private/copy-11.txt' } }, { args: { path: '/mock/private/copy-11.txt' } },
  ])
})

test('offline cleanup requires confirmation, updates storage and ignores stale status for removed copies', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [],
      cloudCopies: [{ id: 'abc-123', sourcePath: 'rclone://test/saved.txt', localPath: '/mock/private/saved.txt',
        originalSize: 8, originalModified: null, originalHash: 'hash', createdAt: 1,
        dirty: false, uploadedPath: null, saveStatus: 'saved', storageBytes: 1234 }],
      cloudStatuses: [{ id: 'abc-123', name: 'saved.txt', sourcePath: 'rclone://test/saved.txt', status: 'saved', message: null, bytes: 0, total: 0, sequence: 1 }],
    }
  })
  await page.goto('/')
  await page.keyboard.press('Control+s')
  await page.getByPlaceholder('Filter settings').fill('cloud')
  await page.getByRole('button', { name: 'Working copies…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Cloud working copies', exact: true })
  await expect(dialog.getByText(/Local storage: 1.2 kB/)).toBeVisible()
  await chooseAction(dialog, 'Actions for rclone://test/saved.txt', 'Remove local copy…')
  await expect(page.getByText(/Close these files in your editor before continuing/)).toBeVisible()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await chooseAction(dialog, 'Actions for rclone://test/saved.txt', 'Remove local copy…')
  await page.getByRole('button', { name: 'Cancel', exact: true }).press('Escape')
  await expect(page.getByRole('button', { name: 'Files closed — move to trash', exact: true })).toHaveCount(0)
  await expect(dialog).toBeVisible()
  await expect(page.locator('.settings-modal')).toBeVisible()
  const calls = () => page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
  }).__BROWSEY_E2E__.calls)
  expect((await calls()).filter(call => call.cmd === 'remove_cloud_working_copies')).toHaveLength(0)
  await chooseAction(dialog, 'Actions for rclone://test/saved.txt', 'Remove local copy…')
  await page.getByRole('button', { name: 'Files closed — move to trash', exact: true }).click()
  await expect(dialog.getByText('No working copies yet.')).toBeVisible()
  await expect(dialog.getByText(/Local storage: 0 B/)).toBeVisible()
  await expect(dialog.getByText(/1 local working copy moved to the trash. Cloud files were kept./)).toBeVisible()
  expect((await calls()).filter(call => call.cmd === 'remove_cloud_working_copies')).toMatchObject([{ args: { ids: ['abc-123'], editorsClosed: true } }])
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('browsey-e2e-cloud-writeback', { detail: {
    id: 'abc-123', name: 'saved.txt', sourcePath: 'rclone://test/saved.txt', status: 'saved', message: null, bytes: 0, total: 0, sequence: 99,
  } })))
  await expect(page.getByRole('button', { name: 'Cloud saves: Saved', exact: true })).toHaveCount(0)
  await expectDisabledAction(dialog, 'Storage actions', 'Clean up saved copies…')
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)
  await expect(page.locator('.settings-modal')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.locator('.settings-modal')).toHaveCount(0)
})

test('bulk cleanup keeps new edits made during confirmation, conflicts and additional recovery files', async ({ page }) => {
  await page.addInitScript(() => {
    const copy = (id: string, saveStatus: string, cleanupBlockedReason: string | null = null) => ({
      id, sourcePath: `rclone://test/${id}.txt`, localPath: `/mock/private/${id}.txt`,
      originalSize: 8, originalModified: null, originalHash: 'hash', createdAt: 1,
      dirty: false, uploadedPath: null, saveStatus, storageBytes: 100, cleanupBlockedReason,
    })
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], cloudCopies: [copy('a', 'saved'), copy('b', 'saved'), copy('c', 'conflict'), copy('d', 'saved', 'Additional recovery files are present; inspect the local folder first')],
    }
  })
  await page.goto('/')
  await page.keyboard.press('Control+s')
  await page.getByPlaceholder('Filter settings').fill('cloud')
  await page.getByRole('button', { name: 'Working copies…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Cloud working copies', exact: true })
  await expectDisabledAction(dialog, 'Actions for rclone://test/c.txt', 'Remove local copy…')
  await expectDisabledAction(dialog, 'Actions for rclone://test/d.txt', 'Remove local copy…')
  await chooseAction(dialog, 'Storage actions', 'Clean up saved copies…')
  await expect(page.getByText(/2 saved working copies/)).toBeVisible()
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: { cloudCopies: Array<{ id: string; dirty: boolean }> } }).__BROWSEY_E2E__
    control.cloudCopies.find(copy => copy.id === 'b')!.dirty = true
  })
  await page.getByRole('button', { name: 'Files closed — move to trash', exact: true }).click()
  await expect(dialog.locator('section')).toHaveCount(3)
  await expect(dialog.getByText('These copies were kept:')).toBeVisible()
  await expect(dialog.getByText(/rclone:\/\/test\/b.txt: New changes or unresolved saves/)).toBeVisible()
  await expect(dialog.getByText(/Local storage: 300 B/)).toBeVisible()
  await expectDisabledAction(dialog, 'Actions for rclone://test/b.txt', 'Remove local copy…')
  const calls = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
  }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'remove_cloud_working_copies')).toMatchObject([{ args: { ids: ['a', 'b'], editorsClosed: true } }])
})

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
  await chooseAction(dialog, 'Actions for rclone://test/report.txt', 'Save as new file')
  await expect(dialog.getByText(/The cloud original changed\. Saved as a new file:/)).toBeVisible()
  await expect(dialog.getByText(/The original and working copy were kept\./)).toBeVisible()
  expect((await calls()).filter(({ cmd }) => cmd === 'upload_cloud_working_copy')).toMatchObject([{ args: { id: '1-2-3' } }])
  await chooseAction(dialog, 'Storage actions', 'Show storage folder')
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
  await dialog.getByRole('combobox', { name: 'Actions for rclone://test/same.txt', exact: true }).click()
  await expect(dialog.getByRole('option', { name: 'Save as new file', exact: true })).not.toHaveAttribute('aria-disabled', 'true')
  await expect(dialog.getByRole('option', { name: 'Open copy', exact: true })).not.toHaveAttribute('aria-disabled', 'true')
  await dialog.getByRole('combobox', { name: 'Actions for rclone://test/same.txt', exact: true }).press('Escape')
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
  await dialog.getByRole('combobox', { name: 'Actions for rclone://test/same.txt', exact: true }).click()
  await expect(dialog.getByRole('option', { name: 'Save as new file', exact: true })).not.toHaveAttribute('aria-disabled', 'true')

})

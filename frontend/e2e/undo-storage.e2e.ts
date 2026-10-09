import { expect, test, type Page } from '@playwright/test'

type Control = {
  calls: Array<{ cmd: string }>
  failCommands?: string[]
  undoStorageHold?: boolean
  recoveryHold?: boolean
  recoveryOriginalPaths?: Record<string, string>
  recoveryOriginalError?: { code: string; message: string }
  recoveryBackups?: { entries: Array<{ id: string; version: string; name: string; kind: string; bytes: number | null; modifiedAt: number | null; blockedReason: string | null }>; incomplete: boolean }
  undoStorage: {
    directory: string; exists: boolean; sessions: number; markedSessions: number
    files: number; logicalBytes: number; incomplete: boolean
  }
}

const openRecoverySettings = async (page: Page) => {
  await page.keyboard.press('Control+s')
  const settings = page.locator('.settings-modal')
  await settings.getByPlaceholder('Filter settings').fill('recovery')
  return settings.locator('.undo-storage')
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = {
      calls: [],
      undoStorage: {
        directory: '/mock/browsey/undo-sessions', exists: true,
        sessions: 3, markedSessions: 1, files: 4, logicalBytes: 8192, incomplete: false,
      },
    }
  })
  await page.goto('/')
})

for (const width of [900, 620]) {
  test(`recovery information and copyable path fit at ${width}px without destructive controls`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 })
    await page.evaluate(() => {
      const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
      control.undoStorage.directory = `/mock/${'long-backup-directory-'.repeat(15)}/undo-sessions`
    })
    const recovery = await openRecoverySettings(page)
    const layout = await recovery.evaluate(element => ({
      top: element.getBoundingClientRect().top,
      labelTop: document.querySelector('.backup-label')!.getBoundingClientRect().top,
    }))
    expect(Math.abs(layout.top - layout.labelTop)).toBeLessThanOrEqual(1)
    for (const name of ['Caches', 'Saved lists']) {
      const actions = page.getByRole('group', { name, exact: true })
      const dimensions = await actions.evaluate(element => ({ client: element.clientWidth, scroll: element.scrollWidth }))
      expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client + 1)
    }
    await expect(recovery.getByRole('status')).toContainText('8.2 kB stored in backups.')
    const path = recovery.getByRole('textbox', { name: 'Undo backup directory' })
    await expect(path).toBeHidden()
    await expect(recovery.getByText('Backups are kept across restarts.', { exact: false })).toBeVisible()
    await expect(recovery.getByText('Stored backups have no automatic expiry.', { exact: false })).toBeHidden()
    const guidance = recovery.locator('summary', { hasText: 'Advanced details' })
    await guidance.focus()
    await guidance.press('Enter')
    await expect(path).toBeVisible()
    await expect(path).toHaveAttribute('readonly', '')
    await path.focus()
    await path.press('Control+a')
    expect(await path.evaluate((element: HTMLInputElement) => element.selectionEnd! - element.selectionStart!))
      .toBe((await path.inputValue()).length)
    const dimensions = await recovery.evaluate(element => ({ client: element.clientWidth, scroll: element.scrollWidth }))
    expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client + 1)
    await expect(recovery.getByText('Stored backups have no automatic expiry.', { exact: false })).toBeVisible()
    await expect(recovery.getByRole('button')).toHaveCount(2)
    const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
    expect(calls.some(call => /^(delete_|clear_|restore_)/.test(call.cmd))).toBe(false)
  })
}

test('incomplete measurements are explicit and not presented as empty storage', async ({ page }) => {
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    Object.assign(control.undoStorage, { incomplete: true, logicalBytes: 0, files: 0 })
  })
  const recovery = await openRecoverySettings(page)
  await expect(recovery.getByRole('status')).toContainText('Partial scan: at least 0 B stored.')
  await expect(recovery.getByRole('status')).toContainText('Some backups could not be inspected.')
  await expect(recovery.getByRole('status')).not.toContainText('No backups found.')
})

test('refresh errors retain and label the previous measurement; retry only inspects storage', async ({ page }) => {
  const recovery = await openRecoverySettings(page)
  await expect(recovery.getByRole('status')).toContainText('8.2 kB stored in backups.')
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['inspect_undo_storage']
  })
  await recovery.getByRole('button', { name: 'Refresh', exact: true }).click()
  await expect(recovery.getByRole('status')).toContainText('Could not inspect backups:')
  await expect(recovery.getByRole('status')).toContainText('Previous measurement is not current.')
  await expect(recovery.getByRole('status')).toContainText('8.2 kB')
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    control.failCommands = []
    control.undoStorage.markedSessions = 0
  })
  await recovery.getByRole('button', { name: 'Refresh', exact: true }).click()
  await expect(recovery.getByRole('status')).not.toContainText('with recovery markers')
  await expect(recovery.getByRole('status')).not.toContainText('Could not inspect')
})

test('Settings omits drag-and-drop prose and keeps recovery details collapsed after reopening', async ({ page }) => {
  await page.keyboard.press('Control+s')
  const settings = page.locator('.settings-modal')
  await expect(settings.getByText('Drag & drop:', { exact: false })).toHaveCount(0)
  await expect(settings.locator('.shortcuts-columns')).toBeVisible()
  const recovery = settings.locator('.undo-storage')
  await expect(recovery.locator('details')).not.toHaveAttribute('open', '')
  await recovery.locator('summary').click()
  await expect(recovery.getByRole('textbox', { name: 'Undo backup directory' })).toBeVisible()
  await recovery.locator('summary').focus()
  await page.keyboard.press('Escape')
  const reopened = await openRecoverySettings(page)
  await expect(reopened.locator('details')).not.toHaveAttribute('open', '')
  await expect(reopened.getByRole('textbox', { name: 'Undo backup directory' })).toBeHidden()
})

test('grouped cleanup buttons retain separate confirmations and never clear on cancellation', async ({ page }) => {
  await openRecoverySettings(page)
  const settings = page.locator('.settings-modal')
  for (const [label, title] of [
    ['Clear thumbnail cache', 'Clear thumbnail cache?'],
    ['Clear cloud file cache', 'Clear cloud file cache?'],
    ['Clear stars', 'Clear all stars?'],
    ['Clear bookmarks', 'Clear all bookmarks?'],
    ['Clear recents', 'Clear all recents?'],
  ]) {
    await settings.getByRole('button', { name: label, exact: true }).click()
    const confirmation = page.getByRole('dialog', { name: title, exact: true })
    await expect(confirmation).toBeVisible()
    await confirmation.getByRole('button', { name: 'Cancel', exact: true }).click()
  }
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
  expect(calls.some(call => /^(delete_|clear_|restore_)/.test(call.cmd))).toBe(false)
})

test('pending inspection disables repeats and closing Settings discards its late result', async ({ page }) => {
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.undoStorageHold = true
  })
  const recovery = await openRecoverySettings(page)
  await expect(recovery.getByRole('button', { name: 'Inspecting backups…' })).toBeDisabled()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'inspect_undo_storage')).toHaveLength(1)
  // Escape in the filter first blurs text entry by design; focus a non-entry control.
  await recovery.locator('summary').focus()
  await page.keyboard.press('Escape')
  await expect(page.locator('.settings-modal')).toBeHidden()
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    control.undoStorageHold = false
    control.undoStorage.logicalBytes = 512
  })
  const reopened = await openRecoverySettings(page)
  await expect(reopened.getByRole('status')).toContainText('512 B stored in backups.')
})

test('offers Recover to only after the original location is unavailable and keeps in-use backups unavailable', async ({ page }) => {
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    control.recoveryHold = true
    control.recoveryOriginalPaths = { 'session-fixture/a/report.txt': '/mock/report.txt' }
    control.recoveryOriginalError = { code: 'recovery_destination_unavailable', message: 'The original location already contains a file. Choose another folder.' }
    control.recoveryBackups = { incomplete: false, entries: [
      { id: 'session-fixture/a/report.txt', version: 'v1', name: 'report.txt', kind: 'file', bytes: 2048, modifiedAt: null, blockedReason: null },
      { id: 'session-live/b/busy.txt', version: 'v2', name: 'busy.txt', kind: 'file', bytes: 4096, modifiedAt: null, blockedReason: 'In use by another Browsey instance.' },
    ] }
  })
  const section = await openRecoverySettings(page)
  await section.getByRole('button', { name: 'Show backups' }).click()
  const backups = page.getByRole('dialog', { name: 'Recovery backups', exact: true })
  await expect(backups.getByRole('button', { name: 'Recover busy.txt', exact: true })).toBeDisabled()
  await expect(backups.getByRole('button', { name: 'Recover report.txt', exact: true })).toHaveText('Recover')
  await backups.getByRole('button', { name: 'Recover report.txt', exact: true }).click()
  const recovery = page.getByRole('dialog', { name: 'Recover to…', exact: true })
  await expect(recovery.getByRole('alert')).toContainText('original location already contains a file')
  await recovery.getByRole('group', { name: 'Destination folders' }).getByRole('button', { name: 'Documents', exact: false }).click()
  await expect(recovery.getByRole('textbox', { name: 'Destination folder' })).toHaveValue('/mock/Documents')
  await recovery.getByRole('button', { name: 'Recover here' }).click()
  await expect(backups.getByRole('button', { name: 'Recover report.txt', exact: true })).toHaveCount(0)
  await expect(recovery.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50')
  await expect(recovery.getByRole('status')).toContainText('1.00 KB / 2.00 KB')
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.recoveryHold = false })
  await expect(backups.getByRole('status')).toContainText('Recovered to /mock/Documents/report.txt')
  await expect(backups.getByRole('button', { name: 'Recover report.txt', exact: true })).toHaveCount(0)
  await backups.getByRole('button', { name: 'Open folder' }).click()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> } }).__BROWSEY_E2E__.calls)
  const restores = calls.filter(call => call.cmd === 'restore_recovery_backup')
  expect(restores).toHaveLength(2)
  expect(restores[0]?.args).toMatchObject({ id: 'session-fixture/a/report.txt', version: 'v1', destinationDir: null })
  expect(restores[1]?.args).toMatchObject({ id: 'session-fixture/a/report.txt', version: 'v1', destinationDir: '/mock/Documents' })
  expect(calls.find(call => call.cmd === 'open_entry')?.args).toEqual({ path: '/mock/Documents' })
  expect(calls.some(call => /^(delete_|clear_)/.test(call.cmd))).toBe(false)
})

test('a current-session image becomes recoverable after its file operation finishes without reopening Browsey', async ({ page }) => {
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    control.recoveryOriginalPaths = { 'session-current/b/photo.jpg': '/mock/photo.jpg' }
    control.recoveryBackups = { incomplete: false, entries: [
      { id: 'session-current/b/photo.jpg', version: 'v1', name: 'photo.jpg', kind: 'file', bytes: 3240352, modifiedAt: null,
        blockedReason: 'File operation in progress. Refresh when it finishes.' },
    ] }
  })
  const section = await openRecoverySettings(page)
  await section.getByRole('button', { name: 'Show backups' }).click()
  const backups = page.getByRole('dialog', { name: 'Recovery backups', exact: true })
  await expect(backups.getByRole('button', { name: 'Recover photo.jpg' })).toBeDisabled()
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    control.recoveryBackups!.entries[0].blockedReason = null
  })
  await backups.getByRole('button', { name: 'Refresh', exact: true }).click()
  await expect(backups.getByRole('button', { name: 'Recover photo.jpg' })).toBeEnabled()
  await backups.getByRole('button', { name: 'Recover photo.jpg' }).click()
  await expect(backups.getByRole('status')).toContainText('Recovered to /mock/photo.jpg')
  await expect(backups.getByRole('button', { name: 'Recover photo.jpg' })).toHaveCount(0)
  await expect(backups.getByText('No backups to recover.', { exact: true })).toBeVisible()
  await backups.getByRole('button', { name: 'Refresh', exact: true }).click()
  await expect(backups.getByRole('button', { name: 'Recover photo.jpg' })).toHaveCount(0)
  await expect(page.getByRole('dialog', { name: 'Recover to…', exact: true })).toHaveCount(0)
  await backups.getByRole('button', { name: 'Open folder' }).click()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> } }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'restore_recovery_backup')).toHaveLength(1)
  expect(calls.find(call => call.cmd === 'restore_recovery_backup')?.args).toMatchObject({ destinationDir: null })
  expect(calls.find(call => call.cmd === 'open_entry')?.args).toEqual({ path: '/mock' })
})

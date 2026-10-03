import { expect, test, type Page } from '@playwright/test'

type Control = {
  calls: Array<{ cmd: string }>
  failCommands?: string[]
  undoStorageHold?: boolean
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
    await expect(recovery.getByRole('status')).toContainText('Last scan: 8.2 kB')
    await expect(recovery.getByRole('status')).toContainText('1 session with recovery markers')
    const path = recovery.getByRole('textbox', { name: 'Undo backup directory' })
    await expect(path).toHaveAttribute('readonly', '')
    await path.focus()
    await path.press('Control+a')
    expect(await path.evaluate((element: HTMLInputElement) => element.selectionEnd! - element.selectionStart!))
      .toBe((await path.inputValue()).length)
    const dimensions = await recovery.evaluate(element => ({ client: element.clientWidth, scroll: element.scrollWidth }))
    expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client + 1)
    const guidance = recovery.locator('summary', { hasText: 'Manual recovery guidance' })
    await guidance.focus()
    await guidance.press('Enter')
    await expect(recovery.getByText('Do not remove markers just to free space', { exact: false })).toBeVisible()
    await expect(recovery.getByText('Finish file operations and close all Browsey windows', { exact: false })).toBeVisible()
    await expect(recovery.getByRole('button')).toHaveCount(1)
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
  await expect(recovery.getByRole('status')).toContainText('Incomplete scan — counted 0 B')
  await expect(recovery.getByRole('status')).toContainText('These are not complete totals.')
  await expect(recovery.getByRole('status')).not.toContainText('No undo storage')
})

test('refresh errors retain and label the previous measurement; retry only inspects storage', async ({ page }) => {
  const recovery = await openRecoverySettings(page)
  await expect(recovery.getByRole('status')).toContainText('Last scan: 8.2 kB')
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['inspect_undo_storage']
  })
  await recovery.getByRole('button', { name: 'Refresh backup information' }).click()
  await expect(recovery.getByRole('status')).toContainText('Could not inspect undo storage:')
  await expect(recovery.getByRole('status')).toContainText('Previous measurement below is not current.')
  await expect(recovery.getByRole('status')).toContainText('8.2 kB')
  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    control.failCommands = []
    control.undoStorage.markedSessions = 0
  })
  await recovery.getByRole('button', { name: 'Refresh backup information' }).click()
  await expect(recovery.getByRole('status')).toContainText('0 sessions with recovery markers')
  await expect(recovery.getByRole('status')).not.toContainText('Could not inspect')
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
  await expect(reopened.getByRole('status')).toContainText('Last scan: 512 B')
})

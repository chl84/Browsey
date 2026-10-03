import { expect, test } from '@playwright/test'

type Control = {
  calls: Array<{ cmd: string }>
  emptyTrashHold?: boolean
  failCommands?: string[]
  trashEntries: Array<{ name: string; path: string; kind: 'file'; iconId: number; size: number; trash_id: string }>
}
const getControl = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = {
      calls: [],
      trashEntries: [{ name: 'discarded.txt', path: '/mock-trash/discarded.txt', kind: 'file', iconId: 12, size: 128, trash_id: 'discarded' }],
    }
  })
  await page.goto('/')
})

test('only Wastebasket offers emptying, with warning and Cancel focused before deletion', async ({ page }) => {
  const trash = page.getByRole('button', { name: 'Wastebasket', exact: true })
  await page.getByRole('button', { name: 'Home', exact: true }).click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: 'Empty Wastebasket…' })).toHaveCount(0)
  await trash.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Empty Wastebasket…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Empty Wastebasket?' })
  await expect(dialog).toBeVisible()
  await expect(dialog).toContainText('permanently deleted. This cannot be undone.')
  await expect(dialog).toContainText('Cloud providers’ trash is not affected.')
  await expect(dialog.getByRole('button', { name: 'Cancel' })).toBeFocused()
  expect((await page.evaluate(getControl)).calls.some(c => c.cmd === 'empty_trash')).toBe(false)
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(trash).toBeFocused()
  expect((await page.evaluate(getControl)).calls.some(c => c.cmd === 'empty_trash')).toBe(false)
  // Opening the menu must not navigate; ordinary left-click still does.
  await expect(page.locator('.row').filter({ hasText: 'notes' })).toBeVisible()
  await trash.click()
  await expect(page.locator('.row').filter({ hasText: 'discarded' })).toBeVisible()
})

test('keyboard context menu and Escape restore focus without deleting', async ({ page }) => {
  const trash = page.getByRole('button', { name: 'Wastebasket', exact: true })
  for (const key of ['Shift+F10', 'ContextMenu']) {
    await trash.focus()
    await page.keyboard.press(key)
    await expect(page.getByRole('menuitem', { name: 'Empty Wastebasket…' })).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(page.getByRole('dialog').getByRole('button', { name: 'Cancel' })).toBeFocused()
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(trash).toBeFocused()
  }
  expect((await page.evaluate(getControl)).calls.some(c => c.cmd === 'empty_trash')).toBe(false)
})

test('confirmed emptying locks the modal then refreshes the Wastebasket listing', async ({ page }) => {
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.emptyTrashHold = true })
  const trash = page.getByRole('button', { name: 'Wastebasket', exact: true })
  await trash.click()
  await expect(page.locator('.row').filter({ hasText: 'discarded' })).toBeVisible()
  await trash.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Empty Wastebasket…' }).click()
  const dialog = page.getByRole('dialog')
  await dialog.getByRole('button', { name: 'Empty Wastebasket', exact: true }).click()
  await expect(dialog.getByRole('button', { name: 'Working...' })).toBeDisabled()
  await expect(dialog.getByRole('button', { name: 'Cancel' })).toBeDisabled()
  await page.keyboard.press('Escape')
  await expect(dialog).toBeVisible()
  await page.locator('.overlay').click({ position: { x: 5, y: 5 } })
  await expect(dialog).toBeVisible()
  expect((await page.evaluate(getControl)).calls.filter(c => c.cmd === 'empty_trash')).toHaveLength(1)
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.emptyTrashHold = false })
  await expect(dialog).toHaveCount(0)
  await expect(page.locator('.row').filter({ hasText: 'discarded' })).toHaveCount(0)
  await expect(page.getByText('Wastebasket emptied', { exact: true })).toBeVisible()
  expect((await page.evaluate(getControl)).calls.filter(c => c.cmd === 'empty_trash')).toHaveLength(1)
})

test('emptying failures report partial-outcome warning and refresh without retry', async ({ page }) => {
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['empty_trash'] })
  const trash = page.getByRole('button', { name: 'Wastebasket', exact: true })
  await trash.click()
  await trash.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Empty Wastebasket…' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Empty Wastebasket', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByText(/Could not empty Wastebasket:/)).toBeVisible()
  await expect(page.getByText(/Some items may already have been permanently deleted/)).toBeVisible()
  await expect(page.locator('.row').filter({ hasText: 'discarded' })).toBeVisible()
  const state = await page.evaluate(getControl)
  expect(state.calls.filter(c => c.cmd === 'empty_trash')).toHaveLength(1)
  expect(state.calls.filter(c => c.cmd === 'list_trash')).toHaveLength(2)
})

test('already-empty Wastebasket succeeds without affecting the current folder', async ({ page }) => {
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.trashEntries = [] })
  await page.getByRole('button', { name: 'Wastebasket', exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Empty Wastebasket…' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Empty Wastebasket', exact: true }).click()
  await expect(page.getByText('Wastebasket emptied', { exact: true })).toBeVisible()
  await expect(page.locator('.row').filter({ hasText: 'notes' })).toBeVisible()
  expect((await page.evaluate(getControl)).calls.some(c => c.cmd === 'list_trash')).toBe(false)
})

test('listing refresh failures do not mislabel successful deletion', async ({ page }) => {
  const trash = page.getByRole('button', { name: 'Wastebasket', exact: true })
  await trash.click()
  await expect(page.locator('.row').filter({ hasText: 'discarded' })).toBeVisible()
  await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.failCommands = ['list_trash'] })
  await trash.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Empty Wastebasket…' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Empty Wastebasket', exact: true }).click()
  await expect(page.getByText('Wastebasket emptied Could not refresh the listing. Press F5 to refresh.', { exact: true })).toBeVisible()
  expect((await page.evaluate(getControl)).calls.filter(c => c.cmd === 'empty_trash')).toHaveLength(1)
})

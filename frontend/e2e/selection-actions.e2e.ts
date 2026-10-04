import { expect, test } from '@playwright/test'

// Playwright's Desktop Chrome profile advertises Windows. These regressions
// exercise Linux/GVFS policy; Windows keeps its explicit permanent-delete flow.
test.use({ userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36' })

type Control = {
  networkDeleteHold?: boolean
  calls: Array<{ cmd: string; args?: Record<string, unknown> }>
}
const control = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__

for (const input of ['keyboard', 'context menu'] as const) {
  test(`${input} copy/cut use the same clipboard modes`, async ({ page }) => {
    await page.addInitScript(() => {
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
        calls: [], selectionActionsFixture: true,
      }
    })
    await page.goto('/')
    const file = page.locator('.row .name', { hasText: 'notes' })
    for (const [mode, key, menu] of [['copy', 'Control+C', 'Copy'], ['cut', 'Control+X', 'Cut']]) {
      if (input === 'keyboard') {
        await file.click()
        await page.keyboard.press(key)
      } else {
        await file.click({ button: 'right' })
        await page.getByRole('menuitem', { name: new RegExp(`^${menu}\\b`) }).click()
      }
      await expect.poll(async () => (await page.evaluate(control)).calls
        .filter(call => call.cmd === 'set_clipboard_cmd').at(-1)?.args).toMatchObject({ paths: ['/mock/notes.txt'], mode })
    }
  })

  test(`${input} unsupported network trash requires confirmation and exposes shared cancellation`, async ({ page }) => {
    await page.addInitScript(() => {
      const folder = '/run/user/1000/gvfs/sftp:host=server/test'
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
        calls: [], selectionActionsFixture: true, startupPath: folder,
        networkTrashSupported: false,
        listingSnapshot: { current: folder, entries: [{
          name: 'large.bin', path: `${folder}/large.bin`, kind: 'file', iconId: 12,
          size: 67108864, network: true,
        }] },
      }
    })
    await page.goto('/')
    const file = page.locator('.row .name', { hasText: 'large' })
    if (input === 'keyboard') {
      await file.click()
      await page.keyboard.press('Delete')
    } else {
      await file.click({ button: 'right' })
      await page.getByRole('menuitem', { name: /Move to wastebasket/ }).click()
    }
    const dialog = page.getByRole('dialog', { name: 'Network trash unavailable', exact: true })
    await expect(dialog).toBeVisible()
    await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeFocused()
    expect((await page.evaluate(control)).calls.filter(call => call.cmd === 'network_delete_entries')
      .map(call => call.args?.confirmed)).toEqual([false])
    await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.networkDeleteHold = true })
    await dialog.getByRole('button', { name: 'Delete', exact: true }).click()
    await expect(dialog).toHaveCount(0)
    const cancel = page.getByRole('button', { name: 'Cancel task', exact: true })
    await expect(cancel).toBeVisible()
    await cancel.click()
    await expect.poll(async () => (await page.evaluate(control)).calls.filter(call => call.cmd === 'cancel_task').length).toBe(1)
    const calls = (await page.evaluate(control)).calls
    const mutation = calls.filter(call => call.cmd === 'network_delete_entries').at(-1)
    expect(mutation?.args).toMatchObject({ trash: true, confirmed: true })
    expect(calls.find(call => call.cmd === 'cancel_task')?.args?.id).toBe(mutation?.args?.progressEvent)
    await page.evaluate(() => { (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.networkDeleteHold = false })
  })
}

import { expect, test } from '@playwright/test'

type Control = {
  windowControlPolicy?: { minimize: boolean; maximize: boolean }
  failCommands?: string[]
  windowActionFailures?: string[]
  windowActions: string[]
  calls: Array<{ cmd: string }>
}

for (const scenario of [
  { name: 'Hyprland', policy: { minimize: false, maximize: false }, minimize: false, maximize: false },
  { name: 'other desktops', policy: { minimize: true, maximize: true }, minimize: true, maximize: true },
  { name: 'independent controls', policy: { minimize: false, maximize: true }, minimize: false, maximize: true },
  { name: 'policy lookup failure', failCommands: ['get_window_control_policy'], minimize: false, maximize: false },
]) {
  test(`titlebar respects ${scenario.name} and retains menu/close`, async ({ page }) => {
    await page.addInitScript((control: Control) => {
      ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = control
    }, { windowControlPolicy: scenario.policy, failCommands: scenario.failCommands, windowActions: [], calls: [] })
    await page.goto('/')
    await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()
    await expect.poll(async () => page.evaluate(() => {
      const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
      return control.calls.filter(call => call.cmd === 'get_window_control_policy').length
    })).toBe(1)
    await expect(page.getByRole('button', { name: 'Minimize window', exact: true })).toHaveCount(scenario.minimize ? 1 : 0)
    await expect(page.getByRole('button', { name: 'Toggle maximize window', exact: true })).toHaveCount(scenario.maximize ? 1 : 0)
    const menu = page.getByRole('button', { name: 'Main menu', exact: true })
    await menu.click()
    await expect(menu).toHaveAttribute('aria-expanded', 'true')
    await page.getByRole('menuitem', { name: 'Refresh', exact: true }).click()
    await expect(menu).toHaveAttribute('aria-expanded', 'false')
    if (scenario.minimize) await page.getByRole('button', { name: 'Minimize window', exact: true }).click()
    if (scenario.maximize) await page.getByRole('button', { name: 'Toggle maximize window', exact: true }).click()
    await page.getByRole('button', { name: 'Close window', exact: true }).click()
    expect(await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.windowActions))
      .toEqual([
        ...(scenario.minimize ? ['minimize'] : []),
        ...(scenario.maximize ? ['toggleMaximize'] : []),
        'close',
      ])
  })
}

test('window-action failures are caught without unhandled runtime errors', async ({ page }) => {
  const errors: string[] = []
  const loggedFailures: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', message => {
    if (message.type() === 'error') loggedFailures.push(message.text())
  })
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = {
      windowActions: [], calls: [], windowActionFailures: ['minimize', 'toggleMaximize', 'close'],
    }
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'Minimize window', exact: true }).click()
  await page.getByRole('button', { name: 'Toggle maximize window', exact: true }).click()
  await page.getByRole('button', { name: 'Close window', exact: true }).click()
  await expect.poll(() => loggedFailures.filter(message => message.includes('Simulated')).length).toBe(3)
  expect(errors).toEqual([])
})

import { expect, test } from '@playwright/test'

type Control = {
  networkConnections: Array<{ uri: string; label: string }>
  networkMountedPaths: Record<string, string>
  networkConnectError?: string
  calls: Array<{ cmd: string; args?: Record<string, unknown> }>
}
const control = () => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__ = {
      networkConnections: [
        { uri: 'sftp://alice@server/', label: 'SFTP (alice@server)' },
        { uri: 'sftp://bob@server/', label: 'SFTP (bob@server)' },
      ],
      networkMountedPaths: { 'sftp://alice@server/': '/mock/Network' }, calls: [],
    }
  })
  await page.goto('/')
  await page.getByRole('complementary').getByRole('button', { name: 'Network', exact: true }).click()
  await expect(page.getByRole('group', { name: 'File list' }).getByRole('button', { name: /SFTP \(alice@server\)/ })).toBeVisible()
})

test('saved addresses list without reconnecting; forgetting one preserves the mount and other account', async ({ page }) => {
  expect((await page.evaluate(control)).calls.some(call => call.cmd === 'connect_network_uri')).toBe(false)
  const list = page.getByRole('group', { name: 'File list' })
  const alice = list.getByRole('button', { name: /SFTP \(alice@server\)/ })
  await alice.click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: 'Disconnect', exact: true })).toBeVisible()
  await page.getByRole('menuitem', { name: 'Forget Connection', exact: true }).click()
  await expect(alice).toHaveCount(0)
  await expect(list.getByRole('button', { name: /SFTP \(bob@server\)/ })).toBeVisible()
  const state = await page.evaluate(control)
  expect(state.networkMountedPaths['sftp://alice@server/']).toBe('/mock/Network')
  expect(state.calls.some(call => call.cmd === 'eject_drive')).toBe(false)
  expect(state.calls.filter(call => call.cmd === 'forget_network_connection')).toHaveLength(1)
})

test('saved offline connection reports a failed login and allows retry to the exact returned path', async ({ page }) => {
  await page.evaluate(() => {
    ;(window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.networkConnectError = 'Authentication failed'
  })
  const bob = page.getByRole('group', { name: 'File list' }).getByRole('button', { name: /SFTP \(bob@server\)/ })
  await bob.click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: 'Disconnect', exact: true })).toHaveCount(0)
  await page.getByRole('menuitem', { name: 'Connect', exact: true }).click()
  await expect(page.getByRole('status')).toContainText('Connect failed: Authentication failed')
  expect((await page.evaluate(control)).calls.some(call => call.cmd === 'list_dir' && call.args?.path === '/mock/Network')).toBe(false)
  await page.evaluate(() => {
    delete (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.networkConnectError
  })
  await bob.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Connect', exact: true }).click()
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'mock', exact: true })).toBeVisible()
  await expect.poll(async () => (await page.evaluate(control)).calls
    .filter(call => call.cmd === 'list_dir' && call.args?.path === '/mock/Network').length).toBe(1)
  expect((await page.evaluate(control)).networkConnections).toHaveLength(2)
})

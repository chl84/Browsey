import { expect, test, type Page } from '@playwright/test'

const calls = (page: Page) => page.evaluate(() => (window as unknown as {
  __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
}).__BROWSEY_E2E__.calls)

const hoverWastebasket = async (page: Page, sourceName: string) => {
  const source = page.locator('.row, .card').filter({ has: page.getByText(sourceName, { exact: true }) }).first()
  const wastebasket = page.getByRole('button', { name: 'Wastebasket', exact: true })
  const bounds = await wastebasket.boundingBox()
  if (!bounds) throw new Error('Missing Wastebasket')
  const point = { clientX: bounds.x + bounds.width / 2, clientY: bounds.y + bounds.height / 2 }
  const transfer = await page.evaluateHandle(() => new DataTransfer())
  await source.dispatchEvent('dragstart', { dataTransfer: transfer })
  await wastebasket.dispatchEvent('dragover', { dataTransfer: transfer, ...point })
  return { wastebasket, transfer, point }
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [] }
  })
})

test('Wastebasket highlights without hover navigation and retains the existing undo/redo route', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('group', { name: 'File list' })).toBeVisible()
  await page.locator('.row').filter({ has: page.getByText('Documents', { exact: true }) }).click()
  await page.locator('.row').filter({ has: page.getByText('notes', { exact: true }) }).click({ modifiers: ['Control'] })
  const { wastebasket, transfer, point } = await hoverWastebasket(page, 'notes')
  await expect(wastebasket).toHaveAttribute('data-drop-active', 'true')
  await expect(page.locator('.ghost')).toHaveText(/Move to trash: 2 items/)
  await page.waitForTimeout(1000)
  expect((await calls(page)).filter(call => call.cmd === 'list_trash')).toHaveLength(0)
  for (let i = 0; i < 5; i++) await wastebasket.dispatchEvent('dragover', { dataTransfer: transfer, ...point })
  expect((await calls(page)).filter(call => call.cmd === 'can_trash_paths')).toHaveLength(1)
  await wastebasket.dispatchEvent('drop', { dataTransfer: transfer, shiftKey: true, ...point })
  await expect.poll(async () => (await calls(page)).filter(call => call.cmd === 'move_to_trash_many')).toMatchObject([
    { args: { paths: ['/mock/Documents', '/mock/notes.txt'] } },
  ])
  await expect(page.locator('.ghost')).toHaveCount(0)
  await expect(wastebasket).not.toHaveAttribute('data-drop-active')
  await page.keyboard.press('Control+z')
  await page.keyboard.press('Control+y')
  await expect.poll(async () => (await calls(page)).filter(call => ['undo_action', 'redo_action'].includes(call.cmd)).map(call => call.cmd)).toEqual(['undo_action', 'redo_action'])
  expect((await calls(page)).filter(call => ['delete_entries', 'purge_trash_items', 'paste_clipboard_cmd'].includes(call.cmd))).toHaveLength(0)
  await transfer.dispose()
})

test('unsupported native/network trash is rejected without permanent-delete confirmation', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(() => { Object.assign((window as unknown as { __BROWSEY_E2E__: object }).__BROWSEY_E2E__, { trashSupported: false }) })
  const { wastebasket, transfer, point } = await hoverWastebasket(page, 'notes')
  await expect.poll(async () => (await calls(page)).some(call => call.cmd === 'can_trash_paths')).toBe(true)
  await expect(page.locator('.ghost')).toHaveText('Cannot drop here')
  await expect(wastebasket).not.toHaveAttribute('data-drop-active')
  await wastebasket.dispatchEvent('drop', { dataTransfer: transfer, ...point })
  await expect(page.getByText('These items cannot be moved to the trash. Nothing was deleted.')).toBeVisible()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  expect((await calls(page)).filter(call => ['move_to_trash_many', 'network_delete_entries', 'delete_entries', 'purge_trash_items'].includes(call.cmd))).toHaveLength(0)
  await transfer.dispose()
})

for (const [remote, supported] of [['Google Disk', true], ['Onedrive', true], ['Nextcloud', false]] as const) {
  test(`${remote} Wastebasket drop honors provider trash support`, async ({ page }) => {
    const current = `rclone://${remote}/folder`
    const path = `${current}/report.txt~object-id`
    await page.addInitScript(({ current, path, supported }) => {
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
        calls: [], startupPath: current,
        listingSnapshot: { current, entries: [{ name: 'report.txt', path, kind: 'file', ext: 'txt', iconId: 12,
          capabilities: { canList: true, canDelete: true, canTrash: supported, canMove: true, canCopy: true,
            canRename: true, canMkdir: true, canUndo: false, canPermissions: false } }] },
      }
    }, { current, path, supported })
    await page.goto('/')
    const { wastebasket, transfer, point } = await hoverWastebasket(page, 'report')
    await expect(page.locator('.ghost')).toHaveText(supported ? /Move to trash: 1 item/ : 'Cannot drop here')
    await wastebasket.dispatchEvent('drop', { dataTransfer: transfer, ...point })
    if (supported) {
      await expect.poll(async () => (await calls(page)).filter(call => call.cmd === 'trash_cloud_entries')).toMatchObject([{ args: { paths: [path] } }])
      await expect(page.getByText('Moved to cloud trash. Restore items from the provider website.')).toBeVisible()
    } else {
      await expect(page.getByText('These items cannot be moved to the trash. Nothing was deleted.')).toBeVisible()
      expect((await calls(page)).some(call => call.cmd === 'trash_cloud_entries')).toBe(false)
    }
    expect((await calls(page)).filter(call => ['delete_cloud_file', 'delete_cloud_dir_recursive', 'move_to_trash_many', 'can_trash_paths'].includes(call.cmd))).toHaveLength(0)
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await transfer.dispose()
  })
}

test('incoming native local files drop onto Wastebasket rather than a virtual paste directory', async ({ page }) => {
  await page.goto('/')
  const target = page.getByRole('button', { name: 'Wastebasket', exact: true })
  const bounds = await target.boundingBox()
  if (!bounds) throw new Error('Missing Wastebasket')
  await page.evaluate(bounds => {
    window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', { detail: {
      type: 'drop', paths: ['/outside/incoming.txt'], position: {
        x: (bounds.x + bounds.width / 2) * devicePixelRatio,
        y: (bounds.y + bounds.height / 2) * devicePixelRatio,
      },
    } }))
  }, bounds)
  await expect.poll(async () => (await calls(page)).filter(call => call.cmd === 'move_to_trash_many')).toMatchObject([{ args: { paths: ['/outside/incoming.txt'] } }])
  expect((await calls(page)).filter(call => call.cmd === 'paste_clipboard_cmd')).toHaveLength(0)
})

test('grid files also use the Wastebasket action', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [], defaultView: 'grid' }
  })
  await page.goto('/')
  await expect(page.locator('.grid .card').first()).toBeVisible()
  const { wastebasket, transfer, point } = await hoverWastebasket(page, 'notes')
  await expect(wastebasket).toHaveAttribute('data-drop-active', 'true')
  await wastebasket.dispatchEvent('drop', { dataTransfer: transfer, ...point })
  await expect.poll(async () => (await calls(page)).filter(call => call.cmd === 'move_to_trash_many')).toMatchObject([{ args: { paths: ['/mock/notes.txt'] } }])
  await transfer.dispose()
})

for (const losesSupport of [false, true]) {
  test(`network Wastebasket drop revalidates trash without a permanent fallback (${losesSupport})`, async ({ page }) => {
    const path = '/run/user/1000/gvfs/sftp:host=server/remote.txt'
    await page.addInitScript(path => {
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
        calls: [], networkTrashSupported: true,
        listingSnapshot: { current: '/mock', entries: [{ path, name: 'remote.txt', ext: 'txt', kind: 'file', iconId: 12, network: true }] },
      }
    }, path)
    await page.goto('/')
    const { wastebasket, transfer, point } = await hoverWastebasket(page, 'remote')
    await expect(wastebasket).toHaveAttribute('data-drop-active', 'true')
    if (losesSupport) await page.evaluate(() => {
      Object.assign((window as unknown as { __BROWSEY_E2E__: object }).__BROWSEY_E2E__, { networkTrashSupported: false })
    })
    await wastebasket.dispatchEvent('drop', { dataTransfer: transfer, ...point })
    await expect.poll(async () => (await calls(page)).filter(call => call.cmd === 'network_delete_entries')).toMatchObject([
      { args: { paths: [path], trash: true, confirmed: false } },
    ])
    if (losesSupport) await expect(page.getByText('Move to trash failed: Network trash is unsupported.')).toBeVisible()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    expect((await calls(page)).filter(call => ['delete_entries', 'move_to_trash_many', 'purge_trash_items'].includes(call.cmd))).toHaveLength(0)
    await transfer.dispose()
  })
}

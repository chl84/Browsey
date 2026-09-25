import { expect, test, type Page, type Locator } from '@playwright/test'

test.use({ deviceScaleFactor: 2 })

const nativeDrop = async (page: Page, paths: string[], target?: Locator) => {
  const bounds = await (target ?? page.locator('.rows, .grid').first()).boundingBox()
  if (!bounds) throw new Error('Missing drop target')
  const point = target ? { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 }
    : { x: bounds.x + bounds.width - 15, y: bounds.y + bounds.height - 15 }
  await page.evaluate(({ paths, point }) => {
    window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', {
      detail: { type: 'drop', paths, position: { x: point.x * devicePixelRatio, y: point.y * devicePixelRatio } },
    }))
  }, { paths, point })
}

const row = (page: Page, name: string) => page.locator('.rows .row').filter({
  has: page.getByText(name, { exact: true }),
})

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [] }
  })
  await page.goto('/')
  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()
})

test('native drop copies its own file and preserves a previously cut selection', async ({ page }) => {
  const notes = row(page, 'notes')
  await notes.click()
  await page.keyboard.press('Control+x')
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  await expect(row(page, 'report')).toBeVisible()
  await expect(notes).toBeVisible()
  const calls = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: unknown }> }
  }).__BROWSEY_E2E__.calls)
  const transfers = calls.filter(call => call.cmd === 'paste_clipboard_cmd')
  expect(transfers).toHaveLength(1)
  expect(transfers[0].args).toMatchObject({
    dest: '/mock', input: { mode: 'copy', paths: ['/mock/Documents/report.txt'] },
  })
  expect(calls.filter(call => call.cmd === 'clear_system_clipboard')).toHaveLength(0)
})

test('another native drop cannot replace a pending conflict operation', async ({ page }) => {
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  await expect(row(page, 'report')).toBeVisible()
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  const dialog = page.locator('.conflict-modal')
  await expect(dialog).toBeVisible()
  await nativeDrop(page, ['/mock/notes.txt'])
  await expect(dialog.locator('.name')).toHaveText('/mock/Documents/report.txt')
  await dialog.getByRole('button', { name: 'Auto-rename', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(row(page, 'report-1')).toBeVisible()
  await expect(row(page, 'notes-1')).toHaveCount(0)
})

test('native drops target the folder under the pointer, not the current directory', async ({ page }) => {
  await nativeDrop(page, ['/mock/notes.txt'], row(page, 'Documents'))
  await expect.poll(async () => page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: { dest?: string } }> }
  }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'paste_clipboard_cmd').map(call => call.args.dest))).toEqual(['/mock/Documents'])
  await row(page, 'Documents').dblclick()
  await expect(row(page, 'notes')).toBeVisible()
})

test('a regular file and an open properties dialog reject native drops', async ({ page }) => {
  await nativeDrop(page, ['/mock/Documents/report.txt'], row(page, 'notes'))
  await row(page, 'notes').click({ button: 'right' })
  await page.getByRole('menuitem', { name: /Properties/ }).click()
  await expect(page.getByRole('dialog')).toBeVisible()
  await nativeDrop(page, ['/mock/Documents/report.txt'])
  const transfers = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string }> }
  }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'paste_clipboard_cmd'))
  expect(transfers).toHaveLength(0)
})

test('internal Ctrl-drop into blank space copies once', async ({ page }) => {
  await row(page, 'Documents').dblclick()
  const transfer = await page.evaluateHandle(() => new DataTransfer())
  await row(page, 'report').dispatchEvent('dragstart', { dataTransfer: transfer })
  const area = page.locator('.rows')
  const bounds = (await area.boundingBox())!
  const event = { dataTransfer: transfer, ctrlKey: true, clientX: bounds.x + bounds.width - 15, clientY: bounds.y + bounds.height - 15 }
  await area.dispatchEvent('dragover', event)
  await area.dispatchEvent('drop', event)
  await area.dispatchEvent('dragend', { dataTransfer: transfer })
  // Copying within the source directory intentionally auto-renames without a dialog.
  await expect(row(page, 'report-1')).toBeVisible()
  await expect(row(page, 'report')).toBeVisible()
  const transfers = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: unknown }> }
  }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'paste_clipboard_cmd'))
  expect(transfers).toHaveLength(1)
  expect(transfers[0].args).toMatchObject({ dest: '/mock/Documents', input: { mode: 'copy', paths: ['/mock/Documents/report.txt'] } })
})

test('grid folders and sidebar Home are native destinations', async ({ page }) => {
  await page.locator('.rows').dispatchEvent('wheel', { ctrlKey: true, deltaY: -120 })
  const folder = page.locator('.card[data-path="/mock/Documents"]')
  await expect(folder).toBeVisible()
  await nativeDrop(page, ['/mock/notes.txt'], folder)
  await expect.poll(async () => page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string }> }
  }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'paste_clipboard_cmd').length)).toBe(1)
  await nativeDrop(page, ['/mock/Documents/report.txt'], page.getByRole('button', { name: 'Home', exact: true }))
  await expect(page.locator('.card[data-path="/mock/report.txt"]')).toBeVisible()
  const targets = await page.evaluate(() => (window as unknown as {
    __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: { dest?: string } }> }
  }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'paste_clipboard_cmd').map(call => call.args.dest))
  expect(targets).toEqual(['/mock/Documents', '/mock'])
})

test('hovering a folder opens it without transferring until a drop', async ({ page }) => {
  const folder = row(page, 'Documents')
  const bounds = (await folder.boundingBox())!
  await page.evaluate(({ x, y }) => {
    window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', {
      detail: { type: 'enter', paths: ['/mock/notes.txt'], position: { x: x * devicePixelRatio, y: y * devicePixelRatio } },
    }))
  }, { x: bounds.x + 25, y: bounds.y + bounds.height / 2 })
  await expect(folder).toHaveAttribute('data-drop-active', 'true')
  await expect(row(page, 'report')).toBeVisible()
  await nativeDrop(page, ['/mock/notes.txt'])
  await expect(row(page, 'notes')).toBeVisible()
  await expect(page.locator('[data-drop-active]')).toHaveCount(0)
})

test('native hover autoscrolls a virtualized grid and stops on leave', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      thumbnailFixture: true, defaultView: 'grid', calls: [],
    }
  })
  await page.reload()
  const grid = page.locator('.grid')
  await expect(grid).toBeVisible()
  const bounds = (await grid.boundingBox())!
  await page.evaluate(({ x, y }) => {
    window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', {
      detail: { type: 'enter', paths: ['/outside/file.jpg'], position: { x: x * devicePixelRatio, y: y * devicePixelRatio } },
    }))
  }, { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height - 5 })
  await expect.poll(() => grid.evaluate(el => el.scrollTop)).toBeGreaterThan(50)
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', { detail: { type: 'leave' } })))
  const stopped = await grid.evaluate(el => el.scrollTop)
  await page.waitForTimeout(150)
  expect(await grid.evaluate(el => el.scrollTop)).toBe(stopped)
})

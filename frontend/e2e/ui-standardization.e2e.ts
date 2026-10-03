import { expect, test, type Locator, type Page } from '@playwright/test'

type Control = { calls: Array<{ cmd: string; args?: Record<string, unknown> }> }
const runtimeErrors = new WeakMap<Page, string[]>()

test.beforeEach(async ({ page }) => {
  const errors: string[] = []
  runtimeErrors.set(page, errors)
  page.on('pageerror', error => errors.push(error.message))
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], archivePassword: 'test secret',
      bookmarks: [{ label: 'Saved documents', path: '/mock/Documents' }],
    }
  })
  await page.goto('/')
  await expect(page.getByRole('group', { name: 'File list' })).toBeVisible()
})

test.afterEach(async ({ page }) => { expect(runtimeErrors.get(page)).toEqual([]) })

const fileMenu = async (page: Page, name: string) => {
  await page.locator('.rows .row').filter({ has: page.getByText('notes', { exact: true }) }).click({ button: 'right' })
  await page.getByRole('menuitem', { name, exact: true }).click()
}

const fieldStyle = (field: Locator) => field.evaluate(node => {
  const style = getComputedStyle(node)
  const bounds = node.getBoundingClientRect()
  return {
    background: style.backgroundColor, color: style.color, border: style.border,
    padding: style.padding, font: style.font, width: bounds.width, height: bounds.height,
  }
})

const expectThemedCheckbox = async (checkbox: Locator) => {
  await expect(checkbox).toHaveCSS('opacity', '0')
  await expect(checkbox.locator('..').locator('.checkbox-indicator')).toBeVisible()
}

for (const appearance of [
  { style: 'Dark', density: 'Cozy' },
  { style: 'Light', density: 'Compact' },
  { style: 'Use system style', density: 'Cozy' },
]) {
  test(`archive controls stay themed and stable with ${appearance.style} / ${appearance.density}`, async ({ page }, testInfo) => {
    await page.keyboard.press('Control+s')
    const settings = page.locator('.settings-modal')
    const filter = settings.getByPlaceholder('Filter settings')
    await filter.fill('style')
    await settings.locator('.combo-btn').click()
    await page.getByRole('option', { name: appearance.style, exact: false }).click()
    if (appearance.style === 'Use system style') {
      await expect(page.locator('html')).toHaveAttribute('data-system-theme', 'Nord')
    }
    await filter.fill('density')
    await settings.locator('.combo-btn').click()
    await page.getByRole('option', { name: appearance.density, exact: true }).click()
    await page.keyboard.press('Escape')

    await fileMenu(page, 'Compress…')
    const compress = page.getByRole('dialog', { name: 'Compress', exact: true })
    const protect = compress.getByRole('checkbox', { name: 'Protect with password' })
    await expectThemedCheckbox(protect)
    await protect.check()
    const show = compress.getByRole('checkbox', { name: 'Show password' })
    await expectThemedCheckbox(show)
    const fields = [compress.getByLabel('Password', { exact: true }), compress.getByLabel('Confirm password')]
    await show.focus()
    const masked = await Promise.all(fields.map(fieldStyle))
    await show.press('Space')
    for (let i = 0; i < fields.length; i++) {
      await expect(fields[i]).toHaveAttribute('type', 'text')
      expect(await fieldStyle(fields[i])).toEqual(masked[i])
      await expect(fields[i]).toHaveClass(/text-field/)
    }
    await show.press('Space')
    await expect(fields[0]).toHaveAttribute('type', 'password')
    await page.screenshot({ path: testInfo.outputPath('compress-controls.png') })
    await compress.getByRole('button', { name: 'Cancel', exact: true }).click()

    await fileMenu(page, 'Extract')
    const extract = page.getByRole('dialog', { name: 'Archive password', exact: true })
    const extractShow = extract.getByRole('checkbox', { name: 'Show password' })
    await expectThemedCheckbox(extractShow)
    await extractShow.focus()
    const password = extract.getByLabel('Password', { exact: true })
    const hidden = await fieldStyle(password)
    await extractShow.press('Space')
    await expect(password).toHaveAttribute('type', 'text')
    expect(await fieldStyle(password)).toEqual(hidden)
  })
}

test('Open with uses the themed checkbox without changing default-save behavior', async ({ page }) => {
  await fileMenu(page, 'Open with…')
  const dialog = page.getByRole('dialog', { name: 'Open with…' })
  const checkbox = dialog.getByRole('checkbox', { name: 'Set as default' })
  await dialog.getByPlaceholder('Filter apps').fill('Beta')
  await expectThemedCheckbox(checkbox)
  await checkbox.focus()
  await checkbox.press('Space')
  await expect(checkbox).toBeChecked()
  await dialog.getByRole('button', { name: 'Open', exact: true }).click()
  await expect(dialog).toBeHidden()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'set_default_app')).toMatchObject([
    { args: { appId: 'beta', contentType: 'text/plain' } },
  ])
})

test('settings radio groups retain arrow navigation, exclusivity and preference saving', async ({ page }) => {
  await page.keyboard.press('Control+s')
  const settings = page.locator('.settings-modal')
  const filter = settings.getByPlaceholder('Filter settings')
  await filter.fill('default view')
  const list = settings.getByRole('radio', { name: 'List', exact: true })
  const grid = settings.getByRole('radio', { name: 'Grid', exact: true })
  await expect(list).toBeChecked()
  await list.focus()
  await expect(list.locator('..').locator('.indicator')).toHaveCSS('outline-style', 'solid')
  await list.press('ArrowRight')
  await expect(grid).toBeFocused()
  await expect(grid).toBeChecked()
  await expect(list).not.toBeChecked()
  await grid.press('ArrowLeft')
  await expect(list).toBeChecked()
  await filter.fill('sort direction')
  const ascending = settings.getByRole('radio', { name: 'Ascending', exact: true })
  const descending = settings.getByRole('radio', { name: 'Descending', exact: true })
  await ascending.focus()
  await ascending.press('ArrowDown')
  await expect(descending).toBeChecked()
  await expect(ascending).not.toBeChecked()
  const calls = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls)
  expect(calls.filter(call => call.cmd === 'store_default_view')).toMatchObject([
    { args: { value: 'grid' } }, { args: { value: 'list' } },
  ])
  expect(calls.filter(call => call.cmd === 'store_sort_direction')).toMatchObject([{ args: { value: 'desc' } }])
})

test('advanced rename radio groups keep disabled placement and update preview payloads', async ({ page }) => {
  const rows = page.locator('.rows .row')
  await rows.nth(0).click()
  await rows.nth(1).click({ modifiers: ['Shift'] })
  await rows.nth(1).click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Rename…', exact: true }).click()
  const dialog = page.locator('.advanced-rename-modal')
  const start = dialog.getByRole('radio', { name: 'Start', exact: true })
  const end = dialog.getByRole('radio', { name: 'End', exact: true })
  await expect(start).toBeDisabled()
  await expect(end).toBeDisabled()
  const none = dialog.getByRole('radio', { name: 'None', exact: true })
  await none.focus()
  await none.press('ArrowRight')
  await expect(dialog.getByRole('radio', { name: 'Numeric', exact: true })).toBeChecked()
  await expect(start).toBeEnabled()
  await start.check()
  await expect(start).toBeChecked()
  await expect(end).not.toBeChecked()
  await expect.poll(() => page.evaluate(() => {
    const calls = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls
    return calls.filter(call => call.cmd === 'preview_rename_entries').at(-1)?.args?.payload
  })).toMatchObject({ sequenceMode: 'numeric', sequencePlacement: 'start' })
})

test('sidebar places, bookmarks and filter buttons have visible keyboard focus', async ({ page }) => {
  await page.mouse.move(0, 0)
  await page.keyboard.press('Tab')
  const place = page.locator('.section-wrapper .nav').first()
  const bookmark = page.locator('.nav.bookmark')
  const filter = page.getByRole('button', { name: 'Show bookmark filter' })
  const remove = page.getByRole('button', { name: 'Remove bookmark', exact: true })
  for (const control of [place, bookmark, filter, remove]) {
    await control.focus()
    await expect(control).toBeFocused()
    await expect(control).toHaveCSS('outline-style', 'solid')
    const width = await control.evaluate(node => parseFloat(getComputedStyle(node).outlineWidth))
    expect(width).toBeGreaterThan(0)
  }
  await expect(remove).toHaveCSS('opacity', '1')
})

test('overwrite uses the destructive button variant and keeps auto-rename as safe initial focus', async ({ page }) => {
  const dropReport = () => page.evaluate(() => {
    const bounds = document.querySelector('.rows')!.getBoundingClientRect()
    window.dispatchEvent(new CustomEvent('browsey-e2e-native-drop', {
      detail: {
        type: 'drop', paths: ['/mock/Documents/report.txt'],
        position: { x: (bounds.right - 15) * devicePixelRatio, y: (bounds.bottom - 15) * devicePixelRatio },
      },
    }))
  })
  await dropReport()
  await expect(page.locator('.rows .row').filter({ has: page.getByText('report', { exact: true }) })).toBeVisible()
  await dropReport()
  const dialog = page.locator('.conflict-modal')
  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Auto-rename' })).toBeFocused()
  await expect(dialog.getByRole('button', { name: 'Overwrite' })).toHaveClass(/\bdanger\b/)
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(dialog).toBeHidden()
})

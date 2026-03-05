import { expect, test } from '@playwright/test'

test('opens a directory from list view with keyboard open', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()

  const rootDirRow = page.locator('.row', {
    has: page.locator('.name', { hasText: 'Documents' }),
  })
  await expect(rootDirRow).toBeVisible()

  await rootDirRow.click()
  await rootDirRow.press('Enter')

  await expect(page.locator('.row .name', { hasText: 'report' })).toBeVisible()
  await expect(page.getByLabel('Path breadcrumbs').getByRole('button', { name: 'Documents' })).toBeVisible()
})

test('wheel assist handles short-list edge clamp and non-cancelable burst fallback', async ({ page }) => {
  await page.goto('/')
  const rows = page.getByRole('grid', { name: 'File list' })
  await expect(rows).toBeVisible()

  const result = await page.evaluate(() => {
    const el = document.querySelector<HTMLElement>('.rows')
    if (!el) {
      return { ok: false as const, reason: 'rows not found' }
    }

    el.style.height = '70px'
    el.style.maxHeight = '70px'
    el.style.flex = '0 0 70px'

    el.scrollTop = 0
    const edgeEvent = new WheelEvent('wheel', { deltaY: -120, deltaMode: 0, cancelable: true })
    const edgeDispatch = el.dispatchEvent(edgeEvent)
    const edgeTop = el.scrollTop

    el.scrollTop = 20
    const e1 = new WheelEvent('wheel', { deltaY: 18, deltaMode: 0, cancelable: true })
    const e2 = new WheelEvent('wheel', { deltaY: 18, deltaMode: 0, cancelable: false })
    const e3 = new WheelEvent('wheel', { deltaY: 18, deltaMode: 0, cancelable: true })
    const d1 = el.dispatchEvent(e1)
    const d2 = el.dispatchEvent(e2)
    const d3 = el.dispatchEvent(e3)

    return {
      ok: true as const,
      edgePrevented: edgeEvent.defaultPrevented,
      edgeDispatch,
      edgeTop,
      burstPrevented: [e1.defaultPrevented, e2.defaultPrevented, e3.defaultPrevented],
      burstDispatch: [d1, d2, d3],
    }
  })

  expect(result.ok).toBeTruthy()
  if (!result.ok) {
    throw new Error(result.reason)
  }
  expect(result.edgePrevented).toBe(true)
  expect(result.edgeDispatch).toBe(false)
  expect(result.edgeTop).toBe(0)
  expect(result.burstPrevented).toEqual([true, false, false])
  expect(result.burstDispatch).toEqual([false, true, true])
})

test('paste failure is surfaced and a following paste can recover', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__?: unknown }).__BROWSEY_E2E__ = {
      systemClipboard: { mode: 'copy', paths: ['/mock/notes.txt'] },
      failCommands: ['paste_clipboard_cmd'],
    }
  })

  await page.goto('/')
  const grid = page.getByRole('grid', { name: 'File list' })
  await expect(grid).toBeVisible()
  await grid.click()

  await page.keyboard.press('Control+V')
  await expect(page.locator('.toast .text')).toHaveText(
    'Paste failed: Simulated paste_clipboard_cmd failure',
  )

  await page.evaluate(() => {
    const control = (window as unknown as { __BROWSEY_E2E__?: { failCommands?: string[] } })
      .__BROWSEY_E2E__
    if (control) {
      control.failCommands = []
    }
  })

  await page.keyboard.press('Control+V')
  await expect(page.locator('.row .name', { hasText: 'notes-1' })).toBeVisible()
})

test('advanced rename modal traps Tab focus and closes on Escape', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('grid', { name: 'File list' })).toBeVisible()

  const pickTargets = async () => {
    const listRows = page.locator('.row')
    if ((await listRows.count()) >= 2) {
      return {
        first: listRows.nth(0),
        second: listRows.nth(1),
      }
    }
    const gridCards = page.locator('.card')
    return {
      first: gridCards.nth(0),
      second: gridCards.nth(1),
    }
  }

  const targets = await pickTargets()
  await expect(targets.first).toBeVisible()
  await expect(targets.second).toBeVisible()

  await targets.first.click()
  await targets.second.click({ modifiers: ['Shift'] })
  await targets.second.click({ button: 'right' })

  await page.getByRole('menuitem', { name: 'Rename…' }).click()

  const modal = page.locator('.modal.advanced-rename-modal')
  const regexInput = page.locator('#advanced-rename-regex')
  await expect(modal).toBeVisible()
  await expect(regexInput).toBeFocused()

  await page.keyboard.press('Shift+Tab')
  const reverseWrappedInside = await modal.evaluate((node) => {
    const active = document.activeElement
    return !!active && node.contains(active) && (active as HTMLElement).id !== 'advanced-rename-regex'
  })
  expect(reverseWrappedInside).toBe(true)
  await page.keyboard.press('Tab')
  await expect(regexInput).toBeFocused()

  for (let i = 0; i < 10; i += 1) {
    await page.keyboard.press('Tab')
    const focusInside = await modal.evaluate((node) => node.contains(document.activeElement))
    expect(focusInside).toBe(true)
  }

  await page.keyboard.press('Shift+Tab')
  const reverseFocusInside = await modal.evaluate((node) => node.contains(document.activeElement))
  expect(reverseFocusInside).toBe(true)

  await page.keyboard.press('Escape')
  await expect(modal).toBeHidden()
})

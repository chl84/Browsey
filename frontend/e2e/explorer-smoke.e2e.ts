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

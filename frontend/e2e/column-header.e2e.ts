import { expect, test } from '@playwright/test'

for (const density of ['Cozy', 'Compact']) {
  for (const width of [90, 70, 150]) {
    test(`SIZE filter and resize handle stay separate with ${density}, ${width}px column`, async ({ page }) => {
      const errors: string[] = []
      page.on('pageerror', error => errors.push(error.message))
      await page.addInitScript(() => {
        ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [] }
      })
      await page.goto('/')
      await page.keyboard.press('Control+s')
      const settings = page.locator('.settings-modal')
      await settings.getByPlaceholder('Filter settings').fill('density')
      await settings.locator('.combo-btn').click()
      await page.getByRole('option', { name: density, exact: true }).click()
      await page.keyboard.press('Escape')
      await expect(settings).toBeHidden()

      const filter = page.getByRole('button', { name: 'Filter Size', exact: true })
      const column = page.locator('.header-cell').filter({ has: filter })
      const handle = column.getByRole('separator')
      const heading = column.getByRole('columnheader')
      await expect(heading).toHaveAttribute('aria-sort', 'none')
      if (width !== 90) {
        const grip = (await handle.boundingBox())!
        const x = grip.x + grip.width / 2
        const y = grip.y + grip.height / 2
        await page.mouse.move(x, y)
        await page.mouse.down()
        await page.mouse.move(x + width - 90, y, { steps: 4 })
        await page.mouse.up()
      }
      await expect.poll(async () => (await column.boundingBox())!.width).toBe(width)

      // Hit testing catches overlap even when a locator retries until clickable.
      const hitTests = await page.locator('.header-row').evaluate(header => {
        const grips = [...header.querySelectorAll('.column-resizer')]
          .map(element => element.getBoundingClientRect())
        return [...header.querySelectorAll('.filter-btn')].map(button => {
          const box = button.getBoundingClientRect()
          return {
            name: button.getAttribute('aria-label'),
            overlaps: grips.some(grip => Math.min(box.right, grip.right) > Math.max(box.left, grip.left)
              && Math.min(box.bottom, grip.bottom) > Math.max(box.top, grip.top)),
            clickable: [box.left + 1, box.left + box.width / 2, box.right - 1]
              .every(x => button.contains(document.elementFromPoint(x, box.top + box.height / 2))),
          }
        })
      })
      expect(hitTests.find(hit => hit.name === 'Filter Size')?.overlaps, 'SIZE filter overlaps a resize handle')
        .toBe(false)
      for (const hit of hitTests) {
        expect(hit.overlaps, `${hit.name} overlaps a resize handle`).toBe(false)
        expect(hit.clickable, `${hit.name} has an obstructed click target`).toBe(true)
      }

      const box = (await filter.boundingBox())!
      for (const x of [box.x + 1, box.x + box.width / 2, box.x + box.width - 1]) {
        await page.mouse.click(x, box.y + box.height / 2)
        await expect(page.locator('.filter-card')).toBeVisible()
        await page.keyboard.press('Escape')
        await expect(page.locator('.filter-card')).toBeHidden()
        expect((await column.boundingBox())!.width).toBe(width)
        await expect(heading).toHaveAttribute('aria-sort', 'none')
      }

      // The handle still resizes, persists the new width, and never opens a filter.
      const grip = (await handle.boundingBox())!
      const x = grip.x + grip.width / 2
      const y = grip.y + grip.height / 2
      await page.mouse.move(x, y)
      await page.mouse.down()
      await page.mouse.move(x + 20, y, { steps: 4 })
      await page.mouse.up()
      await expect.poll(async () => (await column.boundingBox())!.width).toBe(width + 20)
      await expect(page.locator('.filter-card')).toBeHidden()
      const saved = await page.evaluate(() => (window as unknown as {
        __BROWSEY_E2E__: { calls: { cmd: string; args?: { widths?: number[] } }[] }
      }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'store_column_widths').map(call => call.args?.widths?.[3]))
      expect(saved).toEqual(width === 90 ? [width + 20] : [width, width + 20])
      await column.locator('.header-btn').click()
      await expect(heading).toHaveAttribute('aria-sort', 'ascending')
      await column.locator('.header-btn').click()
      await expect(heading).toHaveAttribute('aria-sort', 'descending')
      expect(errors).toEqual([])
    })
  }
}

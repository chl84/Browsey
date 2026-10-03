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
      if (width === 70) {
        await expect.poll(async () => (await column.boundingBox())!.width).toBeGreaterThan(70)
      } else {
        await expect.poll(async () => (await column.boundingBox())!.width).toBe(width)
      }
      const actualWidth = (await column.boundingBox())!.width

      // Right-aligned values keep density-specific padding from the SIZE
      // cell's edge; stars remain left-aligned in their own column.
      const assertSizeAlignment = async () => {
        const alignment = await page.evaluate(() => {
          const label = document.querySelector('.header-cell:nth-child(4) .header-label')!
          const cells = [...document.querySelectorAll<HTMLElement>('.row .col-size')]
          return {
            headerAlign: getComputedStyle(label.closest('.header-btn')!).textAlign,
            stars: [...document.querySelectorAll<HTMLElement>('.row .col-star')].map(cell => {
              const button = cell.querySelector('.star-btn')!
              const glyph = cell.querySelector('.star-glyph')!
              const text = document.createRange()
              text.selectNodeContents(glyph)
              const left = cell.getBoundingClientRect().left
              return { buttonOffset: button.getBoundingClientRect().left - left,
                glyphOffset: text.getBoundingClientRect().left - left,
                targetWidth: button.getBoundingClientRect().width }
            }),
            cells: cells.map(cell => {
              const text = document.createRange()
              text.selectNodeContents(cell)
              return { text: cell.textContent?.trim(), align: getComputedStyle(cell).textAlign,
                padding: cell.getBoundingClientRect().right - text.getBoundingClientRect().right }
            }),
          }
        })
        expect(alignment.headerAlign).toBe('left')
        expect(alignment.cells.map(cell => cell.text)).toEqual(['2 items', '1.3 kB'])
        for (const cell of alignment.cells) {
          expect(cell.align).toBe('right')
          expect(cell.padding, `${cell.text} should have right padding`).toBeCloseTo(density === 'Cozy' ? 8 : 6, 1)
        }
        expect(alignment.stars).toHaveLength(2)
        for (const star of alignment.stars) {
          expect(Math.abs(star.buttonOffset)).toBeLessThan(1)
          expect(Math.abs(star.glyphOffset)).toBeLessThan(1)
          expect(star.targetWidth).toBe(22)
        }
      }
      await assertSizeAlignment()

      // Hit testing catches overlap even when a locator retries until clickable.
      const checkFilterTargets = async () => {
        const hitTests = await page.locator('.header-row').evaluate(header => {
          const grips = [...header.querySelectorAll('.column-resizer')]
            .map(element => element.getBoundingClientRect())
          return [...header.querySelectorAll('.filter-btn')].map(button => {
            const box = button.getBoundingClientRect()
            const ownGrip = button.closest('.header-cell')?.querySelector('.column-resizer')?.getBoundingClientRect()
            const sortButton = button.closest('.header-controls')!.querySelector('.header-btn')!.getBoundingClientRect()
            return {
              name: button.getAttribute('aria-label'),
              resizeGap: ownGrip ? ownGrip.left - box.right : null,
              sortFilterGap: box.left - sortButton.right,
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
          expect(hit.sortFilterGap, `${hit.name} should have fixed spacing from sorting`).toBeCloseTo(5, 1)
          expect(hit.resizeGap!, `${hit.name} should stay clear of its resize handle`).toBeGreaterThanOrEqual(density === 'Cozy' ? 7.9 : 5.9)
          expect(hit.overlaps, `${hit.name} overlaps a resize handle`).toBe(false)
          expect(hit.clickable, `${hit.name} has an obstructed click target`).toBe(true)
        }
      }
      await checkFilterTargets()
      const label = column.locator('.header-label')
      expect(await label.evaluate(element => element.scrollWidth <= element.clientWidth), 'SIZE should not be truncated').toBe(true)

      const box = (await filter.boundingBox())!
      for (const x of [box.x + 1, box.x + box.width / 2, box.x + box.width - 1]) {
        await page.mouse.click(x, box.y + box.height / 2)
        await expect(page.locator('.filter-card')).toBeVisible()
        await page.keyboard.press('Escape')
        await expect(page.locator('.filter-card')).toBeHidden()
        expect((await column.boundingBox())!.width).toBe(actualWidth)
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
      await expect.poll(async () => (await column.boundingBox())!.width).toBe(actualWidth + 20)
      await assertSizeAlignment()
      await checkFilterTargets()
      await expect(page.locator('.filter-card')).toBeHidden()
      const saved = await page.evaluate(() => (window as unknown as {
        __BROWSEY_E2E__: { calls: { cmd: string; args?: { widths?: number[] } }[] }
      }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'store_column_widths').map(call => call.args?.widths?.[3]))
      expect(saved).toEqual(width === 90 ? [actualWidth + 20] : [actualWidth, actualWidth + 20])
      await column.locator('.header-btn').click()
      await expect(heading).toHaveAttribute('aria-sort', 'ascending')
      await column.locator('.header-btn').click()
      await expect(heading).toHaveAttribute('aria-sort', 'descending')
      expect(errors).toEqual([])
    })
  }
}

for (const density of ['Cozy', 'Compact']) {
  test(`complete headers remain visible after restoring tiny widths and shrinking with ${density}`, async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', error => errors.push(error.message))
    await page.addInitScript(() => {
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
        calls: [], columnWidths: [1, 1, 1, 1, 1],
      }
    })
    await page.goto('/')
    await page.keyboard.press('Control+s')
    const settings = page.locator('.settings-modal')
    await settings.getByPlaceholder('Filter settings').fill('density')
    await settings.locator('.combo-btn').click()
    await page.getByRole('option', { name: density, exact: true }).click()
    await page.keyboard.press('Escape')
    await expect(settings).toBeHidden()

    const checkHeaders = async () => {
      await expect.poll(() => page.locator('.header-label').evaluateAll(labels =>
        labels.every(label => label.clientWidth >= label.scrollWidth),
      )).toBe(true)
    }
    await checkHeaders()
    for (const name of ['Name', 'Type', 'Modified', 'Size']) {
      const filter = page.getByRole('button', { name: `Filter ${name}`, exact: true })
      const cell = page.locator('.header-cell').filter({ has: filter })
      const grip = (await cell.getByRole('separator').boundingBox())!
      const x = grip.x + grip.width / 2, y = grip.y + grip.height / 2
      await page.mouse.move(x, y)
      await page.mouse.down()
      await page.mouse.move(0, y, { steps: 4 })
      await page.mouse.up()
      await checkHeaders()
      await filter.click()
      await expect(page.locator('.filter-card')).toBeVisible()
      await page.keyboard.press('Escape')
      await expect(page.locator('.filter-card')).toBeHidden()
      const heading = cell.getByRole('columnheader')
      await cell.locator('.header-btn').click()
      await expect(heading).toHaveAttribute('aria-sort', name === 'Name' ? 'descending' : 'ascending')
      await cell.locator('.header-btn').click()
      await expect(heading).toHaveAttribute('aria-sort', name === 'Name' ? 'ascending' : 'descending')
    }

    const sizeColumn = page.locator('.header-cell').filter({ has: page.getByRole('button', { name: 'Filter Size', exact: true }) })
    const before = (await sizeColumn.boundingBox())!.width
    await page.evaluate(() => { document.body.style.setProperty('--list-header-font-size', '20px') })
    await checkHeaders()
    await expect.poll(async () => (await sizeColumn.boundingBox())!.width).toBeGreaterThan(before)
    // Returning from grid reattaches header measurement without stale observers.
    await page.keyboard.press('Control+g')
    await expect(page.locator('.header-row')).toHaveCount(0)
    await page.keyboard.press('Control+g')
    await expect(page.locator('.header-row')).toBeVisible()
    await checkHeaders()
    expect(errors).toEqual([])
  })
}

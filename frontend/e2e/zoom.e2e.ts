import { expect, test, type Page } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      thumbnailFixture: true, defaultView: 'list', calls: [],
    }
  })
})

const zoom = async (page: Page, deltaY: number) => {
  const consumed = await page.locator('.rows, .grid').evaluate((el, delta) => {
    const event = new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY: delta })
    el.dispatchEvent(event)
    return event.defaultPrevented
  }, deltaY)
  expect(consumed).toBe(true)
  // Wait for the rendered step when testing individual sizes, not a burst.
  await page.waitForTimeout(100)
}

const expectSize = async (page: Page, size: number) => {
  await expect(page.locator('.card img.icon').first()).toHaveCSS('width', `${size}px`)
  await expect(page.locator('.card img.icon').first()).toHaveCSS('height', `${size}px`)
}

test('Ctrl-wheel traverses list and five grid sizes, clamps and preserves selection', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/')
  const list = page.getByRole('group', { name: 'File list' })
  await expect(list).toBeVisible()
  await page.locator('.row').filter({ hasText: 'photo-000' }).click()
  await zoom(page, 120)
  await expect(list).toBeVisible()
  for (const size of [64, 96, 128, 160, 192]) {
    await zoom(page, -120)
    await expectSize(page, size)
  }
  await zoom(page, -120)
  await expectSize(page, 192)
  await expect.poll(() => page.evaluate(() => {
    const calls = (window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: Record<string, unknown> }> } }).__BROWSEY_E2E__.calls
    return calls.some(call => call.cmd === 'get_thumbnail' && Number(call.args?.maxDim) >= 192)
  })).toBe(true)
  for (const size of [160, 128, 96, 64]) {
    await zoom(page, 120)
    await expectSize(page, size)
  }
  await zoom(page, 120)
  await expect(list).toBeVisible()
  await expect(page.locator('.row.selected').filter({ hasText: 'photo-000' })).toHaveCount(1)
  expect(errors).toEqual([])
})

test('normal scrolling is unchanged and zoom retains the visible region', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('group', { name: 'File list' })).toBeVisible()
  await page.locator('.rows').evaluate(el => { el.scrollTop = 1500 })
  await zoom(page, -120)
  await expectSize(page, 64)
  const centrePath = await page.locator('.grid').evaluate(el => {
    const bounds = el.getBoundingClientRect()
    return [...el.querySelectorAll<HTMLElement>('.card')].find(card => {
      const rect = card.getBoundingClientRect()
      return rect.top <= bounds.top + bounds.height / 2 && rect.bottom >= bounds.top + bounds.height / 2
    })?.dataset.path
  })
  expect(centrePath).toBeTruthy()
  await zoom(page, -120)
  await expectSize(page, 96)
  await expect(page.locator(`.card[data-path="${centrePath}"]`)).toBeInViewport()
  const before = await page.locator('.grid').evaluate(el => el.scrollTop)
  await page.locator('.grid').dispatchEvent('wheel', { deltaY: 120, cancelable: true, bubbles: true })
  await expect.poll(() => page.locator('.grid').evaluate(el => el.scrollTop)).toBeGreaterThan(before)
  await expectSize(page, 96)
})

test('native Ctrl-wheel zooms files, not the page, and compact density keeps zoom', async ({ page }) => {
  await page.goto('/')
  const rows = page.locator('.rows')
  await expect(rows).toBeVisible()
  const ratio = await page.evaluate(() => window.devicePixelRatio)
  await rows.hover()
  await page.keyboard.down('Control')
  await page.mouse.wheel(0, -120)
  await page.keyboard.up('Control')
  await expectSize(page, 64)
  expect(await page.evaluate(() => window.devicePixelRatio)).toBe(ratio)
  await expect(page.locator('.grid-viewport')).toHaveCSS('gap', '8px')

  await page.getByRole('button', { name: 'Main menu' }).click()
  await page.getByRole('menuitem', { name: 'Settings…' }).click()
  const settings = page.locator('.settings-modal')
  await expect(settings).toBeVisible()
  await zoom(page, -120)
  await expectSize(page, 64)
  await settings.getByPlaceholder('Filter settings').fill('density')
  await settings.getByRole('button', { name: 'Cozy' }).click()
  await page.getByRole('option', { name: 'Compact' }).click()
  await page.keyboard.press('Escape')
  await expect(settings).toBeHidden()
  await expect(page.locator('.grid-viewport')).toHaveCSS('gap', '6px')
  await expectSize(page, 64)
  await zoom(page, -120)
  await expectSize(page, 96)
})

test('rapid notches reach the final size and reverse back to list without losing selection', async ({ page }) => {
  await page.goto('/')
  await page.locator('.row').filter({ hasText: 'photo-000' }).click()
  const burst = async (delta: number) => page.locator('.rows, .grid').evaluate((el, deltaY) => {
    for (let i = 0; i < 5; i++) {
      const event = new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY })
      el.dispatchEvent(event)
      if (!event.defaultPrevented) throw new Error('Zoom event was not consumed')
    }
  }, delta)
  await burst(-120)
  await expectSize(page, 192)
  await burst(120)
  await expect(page.getByRole('group', { name: 'File list' })).toBeVisible()
  await expect(page.locator('.row.selected').filter({ hasText: 'photo-000' })).toHaveCount(1)
})

test('zoom at the bottom never publishes an empty grid after content shrinks', async ({ page }) => {
  await page.goto('/')
  await zoom(page, -120)
  for (let i = 0; i < 4; i++) await zoom(page, -120)
  await page.locator('.grid').evaluate(el => { el.scrollTop = el.scrollHeight })
  await expect(page.locator('.card[data-path="/mock/photo-099.jpg"]')).toBeVisible()
  for (const size of [160, 128, 96, 64]) {
    await zoom(page, 120)
    await expectSize(page, size)
    const inViewport = await page.locator('.grid').evaluate(el => {
      const bounds = el.getBoundingClientRect()
      return [...el.querySelectorAll('.card')].some(card => {
        const rect = card.getBoundingClientRect()
        return rect.bottom > bounds.top && rect.top < bounds.bottom
      })
    })
    expect(inViewport).toBe(true)
  }
})

test('a continuous zoom gesture skips intermediate thumbnail resolutions', async ({ page }) => {
  await page.goto('/')
  await zoom(page, -120)
  await expect(page.locator('.card img.icon[src^="data:image"]').first()).toBeVisible()
  const dimensions = await page.evaluate(async () => {
    const host = window as unknown as {
      __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: Record<string, unknown> }> }
    }
    const control = host.__BROWSEY_E2E__
    const start = control.calls.length
    for (let i = 0; i < 4; i++) {
      document.querySelector('.grid')!.dispatchEvent(new WheelEvent('wheel', {
        bubbles: true, cancelable: true, ctrlKey: true, deltaY: -120,
      }))
      await new Promise(resolve => setTimeout(resolve, 40))
    }
    return control.calls.slice(start).filter(call => call.cmd === 'get_thumbnail').map(call => call.args.maxDim)
  })
  const ratio = await page.evaluate(() => devicePixelRatio)
  expect(dimensions.every(size => Number(size) === Math.ceil(64 * ratio))).toBe(true)
  await expectSize(page, 192)
  await expect.poll(() => page.evaluate(() => {
    const host = window as unknown as {
      __BROWSEY_E2E__: { calls: Array<{ cmd: string; args: Record<string, unknown> }> }
    }
    return host.__BROWSEY_E2E__.calls.some(call => call.cmd === 'get_thumbnail' && call.args.maxDim === Math.ceil(192 * devicePixelRatio))
  })).toBe(true)
})

test('a coalesced grid-to-list switch captures the anchor before changing grid geometry', async ({ page }) => {
  await page.addInitScript(() => {
    const host = window as unknown as { __BROWSEY_E2E__: { performanceFixture?: { entries: number } } }
    host.__BROWSEY_E2E__.performanceFixture = { entries: 1000 }
  })
  await page.goto('/')
  await page.locator('.rows').evaluate(el => {
    for (let i = 0; i < 5; i++) el.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true, cancelable: true, ctrlKey: true, deltaY: -120,
    }))
  })
  await expectSize(page, 192)
  const anchorPath = await page.locator('.grid').evaluate(async el => {
    el.scrollTop = 1600
    await new Promise(resolve => requestAnimationFrame(resolve))
    await new Promise(resolve => requestAnimationFrame(resolve))
    const bounds = el.getBoundingClientRect()
    const centre = bounds.top + el.clientHeight / 2
    return [...el.querySelectorAll<HTMLElement>('.card')].find(card => {
      const rect = card.getBoundingClientRect()
      return rect.top <= centre && rect.bottom > centre
    })?.dataset.path
  })
  expect(anchorPath).toBeTruthy()
  await page.locator('.grid').evaluate(el => {
    for (let i = 0; i < 5; i++) el.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true, cancelable: true, ctrlKey: true, deltaY: 120,
    }))
  })
  const anchorName = anchorPath!.split('/').at(-1)!.replace(/\.jpg$/, '')
  await expect(page.locator('.row').filter({ hasText: anchorName })).toBeInViewport()
})

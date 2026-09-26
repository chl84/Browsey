import { expect, test } from '@playwright/test'

for (const width of [900, 620]) {
  test(`mount refresh slider keeps usable width at ${width}px`, async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    await page.addInitScript(() => {
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [] }
    })
    await page.setViewportSize({ width, height: 800 })
    await page.goto('/')
    await page.keyboard.press('Control+s')
    const settings = page.locator('.settings-modal')
    await settings.getByPlaceholder('Filter settings').fill('mount refresh')
    const control = settings.locator('.form-control').filter({ hasText: 'Controls how often Browsey rescans' })
    const slider = control.getByRole('slider', { name: 'Mount refresh interval' })
    await expect(slider).toBeVisible()
    const bounds = await slider.boundingBox()
    expect(bounds?.width).toBeGreaterThan(120)
    await expect(slider).toHaveAttribute('aria-describedby', 'settings-mount-refresh-help')
    const helpBounds = await control.locator('#settings-mount-refresh-help').boundingBox()
    expect(helpBounds!.y).toBeGreaterThan(bounds!.y + bounds!.height)

    await slider.focus()
    await slider.press('End')
    await expect(slider).toHaveValue('10000')
    await expect(control.getByText('10000 ms', { exact: true })).toBeVisible()
    await expect(control.locator('.mount-refresh-value')).toHaveCSS('white-space', 'nowrap')
    await slider.press('Home')
    await expect(slider).toHaveValue('500')
    await slider.press('ArrowRight')
    await expect(slider).toHaveValue('600')

    await slider.click({ position: { x: bounds!.width / 2, y: bounds!.height / 2 } })
    const selected = Number(await slider.inputValue())
    expect(selected).toBeGreaterThan(4000)
    expect(selected).toBeLessThan(6500)
    await expect(control.getByText(`${selected} ms`, { exact: true })).toBeVisible()
    const saved = await page.evaluate(() => {
      const calls = (window as unknown as { __BROWSEY_E2E__: { calls: Array<{ cmd: string; args?: { value?: number } }> } }).__BROWSEY_E2E__.calls
      return calls.filter((call) => call.cmd === 'store_mounts_poll_ms').map((call) => call.args?.value)
    })
    expect(saved).toEqual(expect.arrayContaining([10000, 500, 600, selected]))
    expect(errors).toEqual([])
  })
}

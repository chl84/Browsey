import { expect, test } from '@playwright/test'

test('3D model icons load in list and grid without replacing generic file icons', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], listingEntries: [
        { name: 'mesh.stl', path: '/mock/mesh.stl', kind: 'file', ext: 'stl', size: 1024, iconId: 22 },
        { name: 'scene.blend', path: '/mock/scene.blend', kind: 'file', ext: 'blend', size: 2048, iconId: 22 },
        { name: 'unknown.custom', path: '/mock/unknown.custom', kind: 'file', ext: 'custom', size: 10, iconId: 12 },
      ],
    }
  })
  await page.goto('/')
  for (const selector of ['.row', '.card']) {
    const iconFor = (name: string) => page.locator(selector)
      .filter({ hasText: name.split('.')[0] }).locator('img.icon')
    for (const name of ['mesh.stl', 'scene.blend']) {
      const icon = iconFor(name)
      await expect(icon).toHaveAttribute('src', 'icons/scalable/browsey/model_3d_file.svg')
      await expect.poll(() => icon.evaluate(img => (img as HTMLImageElement).naturalWidth)).toBe(128)
    }
    await expect(iconFor('unknown.custom'))
      .toHaveAttribute('src', 'icons/scalable/browsey/file.svg')
    if (selector === '.row') await page.keyboard.press('Control+g')
  }
  expect(errors).toEqual([])
})

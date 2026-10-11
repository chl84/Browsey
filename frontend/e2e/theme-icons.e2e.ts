import { expect, test } from '@playwright/test'

const ristretto = {
  name: 'Ristretto', mode: 'dark', accent: '#f38d70', selection: '#403e41', muted: '#72696a',
  background: '#2c2525', darkBackground: '#211b1b', darkerBackground: '#181414', lighterBackground: '#3d2f2a',
  foreground: '#e6d9db', darkForeground: '#72696a', lightForeground: '#c3b7b8', brightForeground: '#e6d9db',
  red: '#fd6883', yellow: '#f9cc6c', orange: '#fb9a77', green: '#adda78', cyan: '#85dacc', blue: '#f38d70', magenta: '#a8a9eb',
}

test('all icon types share symbols, adapt to themes/selection and keep list/grid dimensions', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.addInitScript(systemTheme => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], systemTheme,
      listingEntries: Array.from({ length: 43 }, (_, iconId) => ({
        name: `Icon ${String(iconId).padStart(2, '0')}`, path: `/mock/icon-${iconId}`, kind: 'file', iconId,
      })),
    }
  }, ristretto)
  await page.goto('/')
  const folder = page.locator('.row[data-path="/mock/icon-10"] svg.icon')
  await expect(folder).toBeVisible()
  await expect(folder).toHaveCSS('width', '18px')
  const symbolCount = await page.locator('.icon-sprite symbol').count()
  expect(symbolCount).toBe(53)
  const colors: string[] = []
  for (const mode of ['dark', 'light', 'system', 'dark'] as const) {
    await page.evaluate(async mode => {
      const path = '/src/features/explorer/theme/themeController.ts'
      const { setThemeMode } = await import(path)
      await setThemeMode(mode)
    }, mode)
    const state = await folder.evaluate(svg => {
      const style = getComputedStyle(svg)
      const box = (svg as SVGSVGElement).getBBox()
      return { color: style.getPropertyValue('--icon-folder').trim(), width: box.width, height: box.height }
    })
    expect(state.width).toBeGreaterThan(0)
    expect(state.height).toBeGreaterThan(0)
    colors.push(state.color)
    expect(await page.locator('.icon-sprite symbol').count()).toBe(symbolCount)
  }
  expect(new Set(colors.slice(0, 3)).size).toBe(3)
  expect(colors[3]).toBe(colors[0])
  expect(colors[2]).toBe(ristretto.accent)
  await page.locator('.row[data-path="/mock/icon-10"]').click()
  const selected = await folder.evaluate(svg => ({
    current: getComputedStyle(svg).getPropertyValue('--icon-folder').trim(),
    expected: getComputedStyle(document.documentElement).getPropertyValue('--icon-selected-folder').trim(),
  }))
  expect(selected.current).toBe(selected.expected)
  const dropped = await folder.evaluate(svg => {
    const row = svg.closest('.row')!
    row.classList.add('drop-target')
    try {
      return {
        current: getComputedStyle(svg).getPropertyValue('--icon-paper').trim(),
        expected: getComputedStyle(document.documentElement).getPropertyValue('--icon-paper').trim(),
      }
    } finally { row.classList.remove('drop-target') }
  })
  expect(dropped.current).toBe(dropped.expected)
  await page.keyboard.press('Control+g')
  const card = page.locator('.card[data-path="/mock/icon-10"] svg.icon')
  await expect(card).toHaveCSS('width', '96px')
  await expect(card).toHaveCSS('height', '96px')
  await expect(card.locator('use')).toHaveAttribute('href', '#browsey-icon-browsey-folder-svg')
  for (const enabled of [true, false]) {
    await page.evaluate(async enabled => {
      document.documentElement.dataset.highContrast = String(enabled)
      const path = '/src/features/explorer/theme/themeController.ts'
      const { setThemeHighContrast } = await import(path)
      setThemeHighContrast(enabled)
    }, enabled)
    await expect(card).toBeVisible()
  }
  expect(errors).toEqual([])
})

test('new standard folder icons render in grid and list with the shared font glyph', async ({ page }) => {
  const folders = ['temp', 'backup', 'archive', 'code', 'config', 'cache', 'font', 'books', 'games', 'screenshots']
  await page.addInitScript(folders => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
      calls: [], defaultView: 'grid',
      listingEntries: [
        ...folders.map((name, index) => ({ name, path: `/mock/${name}`, kind: 'dir', iconId: 33 + index })),
        { name: 'typeface.ttf', path: '/mock/typeface.ttf', kind: 'file', iconId: 28 },
      ],
    }
  }, folders)
  await page.goto('/')
  for (const name of folders) {
    const icon = page.locator(`.card[data-path="/mock/${name}"] svg.icon`)
    await expect(icon.locator('use')).toHaveAttribute('href', `#browsey-icon-browsey-${name}_folder-svg`)
    const bounds = await icon.evaluate(node => {
      const box = (node as SVGSVGElement).getBBox()
      return { width: box.width, height: box.height }
    })
    expect(bounds.width).toBeGreaterThan(0)
    expect(bounds.height).toBeGreaterThan(0)
  }
  const fonts = await page.evaluate(() => {
    const geometry = (name: string) => {
      const glyph = document.querySelector(`#browsey-icon-browsey-${name}-svg [data-icon-glyph="font"]`)!
      return [...glyph.querySelectorAll('path')].map(path => ({ d: path.getAttribute('d'), width: path.getAttribute('stroke-width') }))
    }
    return { file: geometry('font_file'), folder: geometry('font_folder') }
  })
  expect(fonts.folder).toEqual(fonts.file)
  await page.keyboard.press('Control+g')
  await expect(page.locator('.row[data-path="/mock/font"] svg.icon')).toHaveCSS('width', '18px')
  await expect(page.locator('.row[data-path="/mock/font"] svg.icon use')).toHaveAttribute('href', '#browsey-icon-browsey-font_folder-svg')
})

test('a failed thumbnail switches to a themed icon without retrying or retaining a broken image', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { calls: [], thumbnailFixture: true }
  })
  await page.goto('/')
  const first = page.locator('.card[data-path="/mock/photo-000.jpg"]')
  const thumbnail = first.locator('img.icon')
  await expect(thumbnail).toHaveAttribute('src', /^data:image/)
  await thumbnail.evaluate(img => img.dispatchEvent(new Event('error')))
  await expect(first.locator('img.icon')).toHaveCount(0)
  await expect(first.locator('svg.icon use')).toHaveAttribute('href', '#browsey-icon-browsey-file-svg')
  await expect(first.locator('svg.icon')).toHaveCSS('width', '96px')
})

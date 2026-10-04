import { expect, test } from '@playwright/test'

const camera = '/run/user/1000/gvfs/mtp:host=test/DCIM/Camera'
type Photo = { name: string; path: string; kind: 'file'; iconId: number; modified: string | null; size: number | null; network: boolean }
type Control = { listingSnapshot: { current: string; entries: Photo[]; pendingMetadataPaths?: string[] }; thumbnailHold: boolean; calls: Array<{ cmd: string; args?: Record<string, unknown> }> }

test('camera cards retain positions, scroll, selection and thumbnails during metadata and polling updates', async ({ page }) => {
  await page.addInitScript((path) => {
    const entries = Array.from({ length: 180 }, (_, index) => ({ name: `photo-${String(index).padStart(3, '0')}.jpg`, path: `${path}/photo-${String(index).padStart(3, '0')}.jpg`, kind: 'file', iconId: 12, modified: null, size: null, network: true }))
    ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = { startupPath: path, defaultView: 'grid', sortField: 'modified', thumbnailHold: true, calls: [], listingSnapshot: { current: path, entries, pendingMetadataPaths: entries.map(entry => entry.path) } }
  }, camera)
  await page.goto('/')
  const grid = page.getByRole('group', { name: 'File grid' })
  const cards = grid.locator('button.card')
  await expect(cards.first()).toBeVisible()
  await cards.first().click()
  await expect(grid.locator('.card.selected')).toHaveAttribute('data-path', `${camera}/photo-000.jpg`)
  await grid.evaluate(element => { element.scrollTop = 200 })
  await expect.poll(() => grid.evaluate(element => element.scrollTop)).toBeGreaterThan(150)
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  const capture = () => page.evaluate(() => {
    const grid = document.querySelector('.grid')!
    return { scrollTop: grid.scrollTop, selected: grid.querySelector('.card.selected')?.getAttribute('data-path'), cards: Array.from(grid.querySelectorAll<HTMLElement>('.card')).map(card => ({ path: card.dataset.path, x: card.getBoundingClientRect().x, y: card.getBoundingClientRect().y })) }
  })
  const before = await capture()
  await page.evaluate(async () => {
    const control = (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__
    const resolved = control.listingSnapshot.entries.map((entry, index) => ({ ...entry, size: 4096 + index, modified: new Date(Date.UTC(2026, 0, 28) - index * 60_000).toISOString().slice(0, 16).replace('T', ' ') }))
    const { emitMockEvent } = await import('/src/test/mocks/tauri/event.ts')
    emitMockEvent('entry-meta-batch', resolved.slice(0, 20))
    control.listingSnapshot = { current: control.listingSnapshot.current, entries: [...resolved].reverse() }
    control.thumbnailHold = false
    emitMockEvent('dir-changed', control.listingSnapshot.current)
  })
  await expect.poll(async () => (await capture()).cards).toEqual(before.cards)
  await expect.poll(async () => (await capture()).selected).toBe(before.selected)
  // Finish visible/prefetched thumbnails before measuring cache reuse. A single
  // loaded image does not mean the queue has drained.
  await expect.poll(() => page.evaluate(() => {
    const grid = document.querySelector('.grid')!
    const bounds = grid.getBoundingClientRect()
    const visible = Array.from(grid.querySelectorAll<HTMLElement>('.card')).filter(card => {
      const rect = card.getBoundingClientRect()
      return rect.bottom >= bounds.top && rect.top <= bounds.bottom
    })
    return visible.every(card => card.querySelector<HTMLImageElement>('img.icon')?.src.startsWith('data:image'))
  })).toBe(true)
  await expect.poll(() => page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'list_dir').length)).toBeGreaterThanOrEqual(2)
  const thumbnails = await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'get_thumbnail').length)
  // Wait for at least two real polling cycles, rather than only the watcher.
  await expect.poll(() => page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'list_dir').length), { timeout: 15_000 }).toBeGreaterThanOrEqual(4)
  expect((await capture()).cards).toEqual(before.cards)
  expect((await capture()).scrollTop).toBe(before.scrollTop)
  expect((await capture()).selected).toBe(before.selected)
  expect(await page.evaluate(() => (window as unknown as { __BROWSEY_E2E__: Control }).__BROWSEY_E2E__.calls.filter(call => call.cmd === 'get_thumbnail').length)).toBe(thumbnails)
  await page.keyboard.press('F5')
  await expect(cards.first()).not.toHaveAttribute('data-path', before.cards[0].path!)
})

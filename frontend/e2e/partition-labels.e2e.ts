import { expect, test } from '@playwright/test'

for (const density of ['cozy', 'compact']) {
  test(`partition names use separate ellipsized word lines in a narrow ${density} sidebar`, async ({ page }) => {
    await page.addInitScript(() => {
      ;(window as unknown as { __BROWSEY_E2E__: unknown }).__BROWSEY_E2E__ = {
        partitions: [
          { label: 'SAMSUNG Android', path: '/mock/Phone', fs: 'mtp', removable: true },
          { label: 'VeryLongPartitionName Backup Drive', path: '/mock/Backup', fs: 'ext4', sizeBytes: 32_000_000_000 },
          { label: '/', path: '/', fs: 'btrfs', sizeBytes: 509_900_000_000 },
          { label: 'FFBF-D98E', path: '/mock/USB', fs: 'exfat', removable: true, sizeBytes: 32_200_000_000 },
        ], calls: [],
      }
    })
    await page.goto('/')
    const phone = page.getByRole('complementary').getByRole('button', { name: 'SAMSUNG Android', exact: true })
    await expect(phone).toBeVisible()
    await page.evaluate(value => {
      document.body.classList.toggle('density-compact', value === 'compact')
      const sidebar = document.querySelector<HTMLElement>('aside')!
      sidebar.style.width = '185px'
      sidebar.style.setProperty('--font-size-base', '16px')
    }, density)
    await expect(phone.locator('.nav-word')).toHaveText(['SAMSUNG', 'Android'])
    await expect.poll(() => phone.evaluate(button => {
      const words = Array.from(button.querySelectorAll<HTMLElement>('.nav-word'))
      const rects = words.map(word => word.getBoundingClientRect())
      const actions = button.parentElement!.querySelector('.more')!.getBoundingClientRect()
      return {
        clipped: words[0].scrollWidth > words[0].clientWidth,
        separateLines: rects[1].top >= rects[0].bottom,
        clearOfActions: rects.every(rect => rect.right <= actions.left),
        styles: words.map(word => ({ overflow: getComputedStyle(word).textOverflow, whitespace: getComputedStyle(word).whiteSpace })),
      }
    })).toEqual({
      clipped: true, separateLines: true, clearOfActions: true,
      styles: [{ overflow: 'ellipsis', whitespace: 'nowrap' }, { overflow: 'ellipsis', whitespace: 'nowrap' }],
    })
    await expect(page.getByRole('button', { name: 'VeryLongPartitionName Backup Drive 32 GB', exact: true }).locator('.nav-word')).toHaveText(['VeryLongPartitionName', 'Backup', 'Drive'])
    await expect(page.getByRole('button', { name: '/ 509.9 GB', exact: true }).locator('.capacity')).toHaveText('509.9 GB')
    await phone.locator('.nav-label').hover()
    await expect(page.locator('.browsey-tooltip')).toHaveText('SAMSUNG Android')
    await phone.click({ button: 'right' })
    await expect(page.getByRole('menuitem', { name: 'Properties', exact: true })).toBeVisible()
  })
}

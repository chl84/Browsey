import { mount, tick, unmount } from 'svelte'
import { expect, it } from 'vitest'
import BrowseyIcon from './BrowseyIcon.svelte'
import BrowseyIconSprite from './BrowseyIconSprite.svelte'
import { iconNames, iconSprite, iconSymbol } from './iconSprite'

it('includes every bundled Browsey/status icon and keeps definitions unique', () => {
  const document = new DOMParser().parseFromString(`<svg xmlns="http://www.w3.org/2000/svg">${iconSprite}</svg>`, 'image/svg+xml')
  expect(document.querySelector('parsererror')).toBeNull()
  expect(iconNames).toHaveLength(43)
  expect(document.querySelectorAll('symbol')).toHaveLength(iconNames.length)
  const ids = [...document.querySelectorAll('[id]')].map(element => element.id)
  expect(new Set(ids).size).toBe(ids.length)
  for (const name of iconNames) {
    const symbol = document.getElementById(iconSymbol(`icons/scalable/${name}`))!
    expect(symbol.getAttribute('viewBox')).toBe('0 0 128 128')
    expect(symbol.querySelector('[opacity], [fill-opacity], [stroke-opacity], text')).toBeNull()
    expect(symbol.querySelector('[stroke-linejoin="round"], [stroke-linecap="round"]')).toBeNull()
    for (const element of symbol.querySelectorAll('[fill], [stroke], [stop-color]')) {
      for (const property of ['fill', 'stroke', 'stop-color']) {
        const value = element.getAttribute(property)
        if (!value || value === 'none' || value.startsWith('url(')) continue
        expect(value).toMatch(/^var\(--icon-/)
      }
    }
  }
  for (const match of iconSprite.matchAll(/url\(#([^)]+)\)/g)) expect(document.getElementById(match[1])).not.toBeNull()
})

it('references shared symbols, hides decorative icons and labels meaningful status icons', async () => {
  const target = document.createElement('div')
  document.body.append(target)
  const sprite = mount(BrowseyIconSprite, { target })
  const folder = mount(BrowseyIcon, { target, props: { src: '/icons/scalable/browsey/folder.svg', className: 'icon' } })
  const lock = mount(BrowseyIcon, { target, props: { src: '/icons/scalable/status/padlock.svg', label: 'No read permission' } })
  try {
    await tick()
    expect(target.querySelector('svg.icon')?.getAttribute('aria-hidden')).toBe('true')
    expect(target.querySelector('svg.icon use')?.getAttribute('href')).toBe(`#${iconSymbol('icons/scalable/browsey/folder.svg')}`)
    expect(target.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe('No read permission')
    expect(iconSymbol('untrusted-user-file.svg')).toBe(iconSymbol('icons/scalable/browsey/shortcut.svg'))
  } finally { await unmount(lock); await unmount(folder); await unmount(sprite); target.remove() }
})

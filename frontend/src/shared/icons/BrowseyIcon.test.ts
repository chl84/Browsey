import { mount, tick, unmount } from 'svelte'
import { expect, it } from 'vitest'
import BrowseyIcon from './BrowseyIcon.svelte'
import BrowseyIconSprite from './BrowseyIconSprite.svelte'
import { iconNames, iconSprite, iconSymbol } from './iconSprite'

it('includes every bundled Browsey/status icon and keeps definitions unique', () => {
  const document = new DOMParser().parseFromString(`<svg xmlns="http://www.w3.org/2000/svg">${iconSprite}</svg>`, 'image/svg+xml')
  expect(document.querySelector('parsererror')).toBeNull()
  expect(iconNames).toHaveLength(53)
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

it('uses identical symbol geometry and stroke widths in every file/folder pair', () => {
  const document = new DOMParser().parseFromString(`<svg xmlns="http://www.w3.org/2000/svg">${iconSprite}</svg>`, 'image/svg+xml')
  const geometry = (name: string, glyph: string) => {
    const symbol = document.getElementById(iconSymbol(`icons/scalable/browsey/${name}.svg`))!
    const group = symbol.querySelector(`[data-icon-glyph="${glyph}"]`)!
    expect(group, `${name} must use shared ${glyph}`).not.toBeNull()
    const clone = group.cloneNode(true) as Element
    for (const element of clone.querySelectorAll('[fill], [stroke]')) {
      element.removeAttribute('fill')
      element.removeAttribute('stroke')
    }
    return clone.outerHTML
  }
  for (const [glyph, file, folder] of [
    ['font', 'font_file', 'font_folder'], ['code', 'code_file', 'code_folder'],
    ['document', 'document_file', 'document_folder'], ['music', 'audio_file', 'music_folder'],
    ['picture', 'picture_file', 'pictures_folder'], ['video', 'video_file', 'video_folder'],
    ['book', 'ebook_file', 'books_folder'], ['gear', 'config_file', 'config_folder'],
    ['gear', 'executable_file', 'config_folder'], ['box', 'package_file', 'archive_folder'],
  ]) expect(geometry(file, glyph)).toBe(geometry(folder, glyph))
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

import { afterEach, describe, expect, it, vi } from 'vitest'
import { findDropTarget } from './dropTargets'

afterEach(() => { document.body.innerHTML = '' })
const hit = (markup: string) => {
  document.body.innerHTML = markup
  document.elementFromPoint = vi.fn(() => document.querySelector('#hit'))
}
const point = { x: 100, y: 200 }

describe('drop target hit testing', () => {
  it('prefers the explicit nested folder to the background', () => {
    hit('<div data-drop-background><button data-drop-path="/target"><span id="hit">Folder</span></button></div>')
    expect(findDropTarget(point, '/background')?.path).toBe('/target')
  })
  it('uses the background only in a real directory view', () => {
    hit('<div data-drop-background><div id="hit"></div></div>')
    expect(findDropTarget(point, '/background')).toMatchObject({ path: '/background', openOnHover: false })
    expect(findDropTarget(point, null)).toBeNull()
  })
  it.each([
    '<div data-drop-background><button data-drop-path=""><span id="hit">file</span></button></div>',
    '<div role="dialog"><button id="hit" data-drop-path="/target"></button></div>',
    '<button data-drop-path="/target"><span id="hit" data-drop-blocked>remove</span></button>',
    '<div id="hit">Outside file view</div>',
    '<button id="hit" data-drop-path="usb-volume://unmounted"></button>',
  ])('does not silently redirect a rejected target: %s', markup => {
    hit(markup)
    expect(findDropTarget(point, '/background')).toBeNull()
  })
  it.each(['/mnt/usb', 'rclone://remote/folder', 'C:\\Folder', '\\\\server\\share'])('accepts filesystem and cloud target %s', path => {
    hit('<button id="hit"></button>')
    document.querySelector<HTMLElement>('#hit')!.dataset.dropPath = path
    expect(findDropTarget(point, null)?.path).toBe(path)
  })
})

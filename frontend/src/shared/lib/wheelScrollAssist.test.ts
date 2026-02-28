import { afterEach, describe, expect, it } from 'vitest'
import { applyContainedWheelScrollAssist } from './wheelScrollAssist'

const defineScrollableMetrics = (
  el: HTMLElement,
  {
    clientHeight,
    scrollHeight,
    scrollTop = 0,
  }: { clientHeight: number; scrollHeight: number; scrollTop?: number },
) => {
  Object.defineProperty(el, 'clientHeight', { configurable: true, value: clientHeight })
  Object.defineProperty(el, 'scrollHeight', { configurable: true, value: scrollHeight })
  el.scrollTop = scrollTop
}

describe('applyContainedWheelScrollAssist', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('scrolls the nearest scrollable ancestor within the boundary', () => {
    const modal = document.createElement('div')
    const panel = document.createElement('div')
    const content = document.createElement('div')

    modal.style.overflowY = 'auto'
    panel.style.overflowY = 'auto'

    defineScrollableMetrics(modal, { clientHeight: 200, scrollHeight: 200, scrollTop: 0 })
    defineScrollableMetrics(panel, { clientHeight: 100, scrollHeight: 300, scrollTop: 0 })

    panel.appendChild(content)
    modal.appendChild(panel)
    document.body.appendChild(modal)

    const event = new WheelEvent('wheel', { deltaY: 40, deltaMode: 0, cancelable: true })
    Object.defineProperty(event, 'target', { configurable: true, value: content })

    const handled = applyContainedWheelScrollAssist(modal, event)

    expect(handled).toBe(true)
    expect(panel.scrollTop).toBeGreaterThan(0)
    expect(modal.scrollTop).toBe(0)
  })

  it('returns false when no scrollable ancestor exists', () => {
    const modal = document.createElement('div')
    const content = document.createElement('div')

    modal.style.overflowY = 'hidden'
    defineScrollableMetrics(modal, { clientHeight: 200, scrollHeight: 200, scrollTop: 0 })

    modal.appendChild(content)
    document.body.appendChild(modal)

    const event = new WheelEvent('wheel', { deltaY: 40, deltaMode: 0, cancelable: true })
    Object.defineProperty(event, 'target', { configurable: true, value: content })

    const handled = applyContainedWheelScrollAssist(modal, event)

    expect(handled).toBe(false)
    expect(event.defaultPrevented).toBe(false)
  })
})

import { afterEach, describe, expect, it } from 'vitest'
import { applyWheelScrollAssist } from './wheelScrollHelper'

const createScrollElement = ({
  clientHeight = 100,
  scrollHeight = 280,
  scrollTop = 0,
  lineHeight = '20px',
  paddingTop = '0px',
  paddingBottom = '0px',
}: {
  clientHeight?: number
  scrollHeight?: number
  scrollTop?: number
  lineHeight?: string
  paddingTop?: string
  paddingBottom?: string
} = {}) => {
  const el = document.createElement('div')
  el.style.lineHeight = lineHeight
  el.style.paddingTop = paddingTop
  el.style.paddingBottom = paddingBottom
  Object.defineProperty(el, 'clientHeight', { configurable: true, value: clientHeight })
  Object.defineProperty(el, 'scrollHeight', { configurable: true, value: scrollHeight })
  el.scrollTop = scrollTop
  document.body.appendChild(el)
  return el
}

describe('wheelScrollHelper', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('keeps custom ownership at edge clamp instead of silently falling back', () => {
    const el = createScrollElement({ scrollTop: 0, scrollHeight: 260, clientHeight: 100 })
    const event = new WheelEvent('wheel', { deltaY: -60, deltaMode: 0, cancelable: true })

    const handled = applyWheelScrollAssist(el, event)

    expect(handled).toBe(true)
    expect(event.defaultPrevented).toBe(true)
    expect(el.scrollTop).toBe(0)
  })

  it('uses deterministic fallback when a non-cancelable event appears in a burst', () => {
    const el = createScrollElement({ scrollTop: 40, scrollHeight: 260, clientHeight: 100 })

    const e1 = new WheelEvent('wheel', { deltaY: 18, deltaMode: 0, cancelable: true })
    const e2 = new WheelEvent('wheel', { deltaY: 18, deltaMode: 0, cancelable: false })
    const e3 = new WheelEvent('wheel', { deltaY: 18, deltaMode: 0, cancelable: true })

    const h1 = applyWheelScrollAssist(el, e1)
    const h2 = applyWheelScrollAssist(el, e2)
    const h3 = applyWheelScrollAssist(el, e3)

    expect(h1).toBe(true)
    expect(e1.defaultPrevented).toBe(true)
    expect(h2).toBe(false)
    expect(h3).toBe(false)
  })

  it('normalizes deltaMode line and page values', () => {
    const lineEl = createScrollElement({ scrollTop: 0, scrollHeight: 260, clientHeight: 100, lineHeight: '24px' })
    const lineEvent = new WheelEvent('wheel', { deltaY: 1, deltaMode: 1, cancelable: true })
    const handledLine = applyWheelScrollAssist(lineEl, lineEvent)

    expect(handledLine).toBe(true)
    expect(lineEl.scrollTop).toBeGreaterThan(12)

    const pageEl = createScrollElement({ scrollTop: 0, scrollHeight: 320, clientHeight: 100, lineHeight: '24px' })
    const pageEvent = new WheelEvent('wheel', { deltaY: 1, deltaMode: 2, cancelable: true })
    const handledPage = applyWheelScrollAssist(pageEl, pageEvent)

    expect(handledPage).toBe(true)
    expect(pageEl.scrollTop).toBeGreaterThan(90)
  })

  it('increases gain across rapid bursts with similar deltas', () => {
    const el = createScrollElement({ scrollTop: 20, scrollHeight: 280, clientHeight: 100 })

    const e1 = new WheelEvent('wheel', { deltaY: 10, deltaMode: 0, cancelable: true })
    const beforeFirst = el.scrollTop
    const handledFirst = applyWheelScrollAssist(el, e1)
    const firstStep = el.scrollTop - beforeFirst

    const e2 = new WheelEvent('wheel', { deltaY: 10, deltaMode: 0, cancelable: true })
    const beforeSecond = el.scrollTop
    const handledSecond = applyWheelScrollAssist(el, e2)
    const secondStep = el.scrollTop - beforeSecond

    expect(handledFirst).toBe(true)
    expect(handledSecond).toBe(true)
    expect(firstStep).toBeGreaterThan(0)
    expect(secondStep).toBeGreaterThanOrEqual(firstStep)
  })

  it('handles long ranges with default config (always strategy)', () => {
    const el = createScrollElement({ scrollTop: 0, scrollHeight: 2200, clientHeight: 100 })
    const event = new WheelEvent('wheel', { deltaY: 60, deltaMode: 0, cancelable: true })

    const handled = applyWheelScrollAssist(el, event)

    expect(handled).toBe(true)
    expect(event.defaultPrevented).toBe(true)
    expect(el.scrollTop).toBeGreaterThan(0)
  })
})

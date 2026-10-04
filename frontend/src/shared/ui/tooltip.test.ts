import { afterEach, describe, expect, it, vi } from 'vitest'
import { tooltip } from './tooltip'

describe('tooltip', () => {
  afterEach(() => {
    vi.useRealTimers()
    document.body.innerHTML = ''
  })

  it('shows after delay and hides on mouseleave', () => {
    vi.useFakeTimers()
    const node = document.createElement('button')
    document.body.appendChild(node)

    const action = tooltip(node, 'Copy path')
    node.dispatchEvent(new MouseEvent('mouseenter', { bubbles: true }))

    expect(document.querySelector('.browsey-tooltip')).toBeNull()

    vi.advanceTimersByTime(750)

    const tooltipEl = document.querySelector('.browsey-tooltip')
    expect(tooltipEl).not.toBeNull()
    expect(tooltipEl?.textContent).toBe('Copy path')

    node.dispatchEvent(new MouseEvent('mouseleave', { bubbles: true }))
    expect(document.querySelector('.browsey-tooltip')).toBeNull()
    action.destroy()
  })

  it('updates text while visible', () => {
    vi.useFakeTimers()
    const node = document.createElement('button')
    document.body.appendChild(node)

    const action = tooltip(node, 'First')
    node.dispatchEvent(new FocusEvent('focus', { bubbles: true }))
    vi.advanceTimersByTime(750)
    expect(document.querySelector('.browsey-tooltip')?.textContent).toBe('First')

    action.update('Second')
    expect(document.querySelector('.browsey-tooltip')?.textContent).toBe('Second')
    action.destroy()
  })

  it.each([0, 750])('dismisses tooltips with Escape after %s ms without consuming the key', (elapsed) => {
    vi.useFakeTimers()
    const node = document.createElement('button')
    document.body.appendChild(node)
    const action = tooltip(node, 'Copy path')

    try {
      node.focus()
      vi.advanceTimersByTime(elapsed)
      const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
      const onKeydown = vi.fn()
      window.addEventListener('keydown', onKeydown, { once: true })
      node.dispatchEvent(event)
      vi.advanceTimersByTime(750)

      expect(document.querySelector('.browsey-tooltip')).toBeNull()
      expect(document.activeElement).toBe(node)
      expect(event.defaultPrevented).toBe(false)
      expect(onKeydown).toHaveBeenCalledOnce()
    } finally {
      action.destroy()
    }
  })

  it('keeps a hovered tooltip visible for other keys and dismisses it with Escape', () => {
    vi.useFakeTimers()
    const node = document.createElement('button')
    document.body.appendChild(node)
    const action = tooltip(node, 'Copy path')

    try {
      node.dispatchEvent(new MouseEvent('mouseenter'))
      vi.advanceTimersByTime(750)
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
      expect(document.querySelector('.browsey-tooltip')?.textContent).toBe('Copy path')

      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
      expect(document.querySelector('.browsey-tooltip')).toBeNull()
    } finally {
      action.destroy()
    }
  })

  it('removes tooltip on destroy', () => {
    vi.useFakeTimers()
    const node = document.createElement('button')
    document.body.appendChild(node)

    const action = tooltip(node, 'Tooltip text')
    node.dispatchEvent(new MouseEvent('mouseenter', { bubbles: true }))
    vi.advanceTimersByTime(750)
    expect(document.querySelector('.browsey-tooltip')).not.toBeNull()

    action.destroy()
    expect(document.querySelector('.browsey-tooltip')).toBeNull()
  })
})

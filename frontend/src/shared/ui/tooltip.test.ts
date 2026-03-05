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

    tooltip(node, 'Copy path')
    node.dispatchEvent(new MouseEvent('mouseenter', { bubbles: true }))

    expect(document.querySelector('.browsey-tooltip')).toBeNull()

    vi.advanceTimersByTime(750)

    const tooltipEl = document.querySelector('.browsey-tooltip')
    expect(tooltipEl).not.toBeNull()
    expect(tooltipEl?.textContent).toBe('Copy path')

    node.dispatchEvent(new MouseEvent('mouseleave', { bubbles: true }))
    expect(document.querySelector('.browsey-tooltip')).toBeNull()
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


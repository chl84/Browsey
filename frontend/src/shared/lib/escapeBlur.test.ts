import { describe, expect, it } from 'vitest'
import { blurTextEntryTargetOnEscape } from './escapeBlur'

describe('blurTextEntryTargetOnEscape', () => {
  it('blurs a focused text input on Escape', () => {
    const input = document.createElement('input')
    input.type = 'text'
    document.body.appendChild(input)
    input.focus()

    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
    Object.defineProperty(event, 'target', { value: input })

    const handled = blurTextEntryTargetOnEscape(event)

    expect(handled).toBe(true)
    expect(document.activeElement).not.toBe(input)
    document.body.removeChild(input)
  })

  it('does not intercept combo search inputs', () => {
    const wrap = document.createElement('div')
    wrap.className = 'combo-search-wrap'
    const input = document.createElement('input')
    input.type = 'text'
    wrap.appendChild(input)
    document.body.appendChild(wrap)
    input.focus()

    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
    Object.defineProperty(event, 'target', { value: input })

    const handled = blurTextEntryTargetOnEscape(event)

    expect(handled).toBe(false)
    expect(document.activeElement).toBe(input)
    document.body.removeChild(wrap)
  })
})

import { mount, tick, unmount } from 'svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import ComboBox from './ComboBox.svelte'

const options = Array.from({ length: 40 }, (_, i) => ({ value: `${i}`, label: `Option ${i}` }))
const components: ReturnType<typeof mount>[] = []

const flushDropdown = async () => {
  await tick()
  // Visibility adjustments wait for the newly rendered options and placement.
  await tick()
}

const render = async (searchable: boolean, value = '0') => {
  components.push(mount(ComboBox, { target: document.body, props: { options, value, searchable } }))
  await tick()
  const button = document.querySelector<HTMLButtonElement>('.combo-btn')!
  button.focus()
  button.click()
  await flushDropdown()
  return button
}

const press = async (target: Element, key: string) => {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })
  target.dispatchEvent(event)
  await flushDropdown()
  return event
}

// jsdom has no layout. Model a four-row viewport and the actual list scrollTop.
const mockListLayout = () => {
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockImplementation(function (this: HTMLElement) {
    return this.classList.contains('combo-list') ? 96 : 0
  })
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    if (this.classList.contains('combo-list')) return new DOMRect(0, 120, 200, 96)
    if (this.getAttribute('role') === 'option') {
      const list = this.parentElement!
      const index = Array.from(list.children).indexOf(this)
      return new DOMRect(0, 120 + index * 24 - list.scrollTop, 200, 24)
    }
    return new DOMRect()
  })
}

const expectActiveVisible = (label: string) => {
  const list = document.querySelector<HTMLElement>('.combo-list')!
  const active = list.querySelector<HTMLElement>('.active')!
  expect(active.textContent?.trim()).toBe(label)
  expect(active.getBoundingClientRect().top).toBeGreaterThanOrEqual(list.getBoundingClientRect().top)
  expect(active.getBoundingClientRect().bottom).toBeLessThanOrEqual(list.getBoundingClientRect().bottom)
}

afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

describe.each([false, true])('ComboBox searchable=%s', (searchable) => {
  it('consumes Escape only while open and restores focus to the trigger', async () => {
    const button = await render(searchable)
    const target = searchable ? document.querySelector('.combo-search')! : button
    const bubbled = vi.fn()
    document.addEventListener('keydown', bubbled)
    try {
      const first = await press(target, 'Escape')
      expect(first.defaultPrevented).toBe(true)
      expect(bubbled).not.toHaveBeenCalled()
      expect(button.getAttribute('aria-expanded')).toBe('false')
      expect(document.activeElement).toBe(button)
      const second = await press(button, 'Escape')
      expect(second.defaultPrevented).toBe(false)
      expect(bubbled).toHaveBeenCalledOnce()
    } finally {
      document.removeEventListener('keydown', bubbled)
    }
  })

  it('reveals the selected option on opening and keeps wrapping arrow navigation visible', async () => {
    mockListLayout()
    const button = await render(searchable, '39')
    const target = searchable ? document.querySelector('.combo-search')! : button
    expectActiveVisible('Option 39')
    await press(target, 'ArrowDown')
    expectActiveVisible('Option 0')
    for (let i = 0; i < 12; i++) await press(target, 'ArrowDown')
    expectActiveVisible('Option 12')
    for (let i = 0; i < 12; i++) await press(target, 'ArrowUp')
    expectActiveVisible('Option 0')
    await press(target, 'ArrowUp')
    expectActiveVisible('Option 39')
    for (let i = 0; i < 8; i++) await press(target, 'ArrowUp')
    expectActiveVisible('Option 31')
    await press(target, 'Enter')
    expect(button.textContent).toContain('Option 31')
    button.click()
    await flushDropdown()
    expectActiveVisible('Option 31')
  })

  it('does not scroll or move focus for an already visible option', async () => {
    mockListLayout()
    const button = await render(searchable)
    const target = searchable ? document.querySelector('.combo-search')! : button
    await press(target, 'ArrowDown')
    expectActiveVisible('Option 1')
    expect(document.querySelector('.combo-list')!.scrollTop).toBe(0)
    expect(document.activeElement).toBe(target)
  })
})

describe('ComboBox action selection', () => {
  const renderActions = async () => {
    const change = vi.fn()
    components.push(mount(ComboBox, { target: document.body, props: {
      options: [{ value: 'blocked', label: 'Blocked', disabled: true }, { value: 'open', label: 'Open' },
        { value: 'remove', label: 'Remove', disabled: true }, { value: 'folder', label: 'Show folder' }],
      value: '', placeholder: 'Actions…', ariaLabel: 'Copy actions', resetOnSelect: true,
    }, events: { change } }))
    await tick()
    const button = document.querySelector<HTMLButtonElement>('.combo-btn')!
    button.focus()
    return { button, change }
  }
  it('skips disabled actions with the keyboard and retains the action placeholder after selection', async () => {
    const { button, change } = await renderActions()
    await press(button, 'ArrowDown')
    expect(document.querySelector('.active')!.textContent).toBe('Open')
    await press(button, 'ArrowDown')
    expect(document.querySelector('.active')!.textContent).toBe('Show folder')
    await press(button, 'Enter')
    expect(change).toHaveBeenCalledOnce()
    expect(change.mock.calls[0][0].detail).toBe('folder')
    expect(button.textContent).toContain('Actions…')
    expect(button.getAttribute('aria-expanded')).toBe('false')
    expect(document.activeElement).toBe(button)
  })
  it('ignores pointer selection of disabled actions and permits repeating the same action', async () => {
    const { button, change } = await renderActions()
    button.click(); await flushDropdown()
    document.querySelector('[aria-disabled="true"]')!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }))
    await flushDropdown()
    expect(change).not.toHaveBeenCalled()
    expect(button.getAttribute('aria-expanded')).toBe('true')
    for (let count = 0; count < 2; count++) {
      if (count) { button.click(); await flushDropdown() }
      document.querySelectorAll('[role="option"]')[1].dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }))
      await flushDropdown()
      expect(button.textContent).toContain('Actions…')
    }
    expect(change).toHaveBeenCalledTimes(2)
  })
  it('dismisses the option list on Tab without intercepting normal focus movement', async () => {
    const { button } = await renderActions()
    button.click(); await flushDropdown()
    const tab = await press(button, 'Tab')
    expect(tab.defaultPrevented).toBe(false)
    expect(button.getAttribute('aria-expanded')).toBe('false')
  })
})

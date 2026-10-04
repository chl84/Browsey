import { mount, tick, unmount } from 'svelte'
import { expect, it, vi } from 'vitest'
import SettingsModal from './SettingsModal.svelte'

vi.mock('@/shared/lib/tauri', () => ({ invoke: vi.fn(), convertFileSrc: vi.fn() }))

it('keeps Settings open on the first Escape from a dropdown, then closes on the second', async () => {
  const onClose = vi.fn()
  const onChangeDensity = vi.fn()
  const component = mount(SettingsModal, {
    target: document.body,
    props: { open: true, initialFilter: 'Density', onClose, onChangeDensity },
  })
  try {
    await tick()
    const button = document.querySelector<HTMLButtonElement>('.combo-btn')!
    button.focus()
    button.click()
    await tick()
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await tick()
    expect(onClose).not.toHaveBeenCalled()
    expect(button.getAttribute('aria-expanded')).toBe('false')
    expect(document.activeElement).toBe(button)
    expect(onChangeDensity).not.toHaveBeenCalled()
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await tick()
    expect(onClose).toHaveBeenCalledOnce()
  } finally {
    await unmount(component)
    document.body.innerHTML = ''
  }
})

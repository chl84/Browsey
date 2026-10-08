import { mount, unmount, tick } from 'svelte'
import { expect, it } from 'vitest'
import ModalShell from './ModalShell.svelte'

it('uses an explicit accessible name without also labelling the dialog from its header', async () => {
  const target = document.createElement('div'); document.body.append(target)
  const modal = mount(ModalShell, { target, props: { open: true, title: 'Rich header', accessibleName: 'Properties' } })
  await tick()
  const dialog = target.querySelector('[role="dialog"]')!
  expect(dialog.getAttribute('aria-label')).toBe('Properties')
  expect(dialog.hasAttribute('aria-labelledby')).toBe(false)
  expect(dialog.textContent).toContain('Rich header')
  await unmount(modal); target.remove()
})

it('keeps focus on a stable collection while the old trigger awaits asynchronous removal', async () => {
  const collection = document.createElement('div'); collection.tabIndex = 0
  const trigger = document.createElement('button'), target = document.createElement('div')
  collection.append(trigger); document.body.append(collection, target); trigger.focus()
  const modal = mount(ModalShell, { target, props: { open: true, restoreFocusToAncestor: true } })
  await tick(); await tick(); await unmount(modal); await tick(); await tick()
  const focusedAtClose = document.activeElement
  trigger.remove()
  const focusedAfterRefresh = document.activeElement
  collection.remove(); target.remove()
  expect(focusedAtClose).toBe(collection)
  expect(focusedAfterRefresh).toBe(collection)
})

it('restores the original focusable collection when the dialog trigger was removed', async () => {
  const collection = document.createElement('div')
  collection.tabIndex = 0
  const trigger = document.createElement('button')
  collection.append(trigger); document.body.append(collection); trigger.focus()
  const target = document.createElement('div'); document.body.append(target)
  const modal = mount(ModalShell, { target, props: { open: true } })
  await tick(); await tick()
  trigger.remove()
  await unmount(modal); await tick(); await tick()
  expect(document.activeElement).toBe(collection)
  target.remove(); collection.remove()
})

it('does not steal focus from another connected control during close', async () => {
  const trigger = document.createElement('button'), other = document.createElement('button'), target = document.createElement('div')
  document.body.append(trigger, other, target); trigger.focus()
  const modal = mount(ModalShell, { target, props: { open: true, restoreFocusToAncestor: true } })
  await tick(); await tick(); other.focus()
  await unmount(modal); await tick(); await tick()
  expect(document.activeElement).toBe(other)
  trigger.remove(); other.remove(); target.remove()
})

it.each(['header', 'backdrop', 'list', 'top edge', 'bottom edge'])(
  'contains a nested dialog wheel event over its %s', async area => {
    const target = document.createElement('div')
    document.body.append(target)
    const outer = mount(ModalShell, { target, props: { open: true, title: 'Settings' } })
    await tick()
    const outerDialog = target.querySelector<HTMLElement>('[role="dialog"]')!
    const settingsPanel = document.createElement('div')
    settingsPanel.style.overflowY = 'auto'
    Object.defineProperties(settingsPanel, {
      clientHeight: { value: 200 }, scrollHeight: { value: 800 },
    })
    settingsPanel.scrollTop = 100
    outerDialog.append(settingsPanel)
    const inner = mount(ModalShell, { target: settingsPanel, props: { open: true, title: 'Cloud working copies' } })
    await tick()
    try {
      const dialog = settingsPanel.querySelector<HTMLElement>('[role="dialog"]')!
      const list = document.createElement('div')
      list.style.overflowY = 'auto'
      Object.defineProperties(list, {
        clientHeight: { value: 100 }, scrollHeight: { value: 400 },
      })
      dialog.append(list)
      list.scrollTop = area === 'top edge' ? 0 : area === 'bottom edge' ? 300 : 100
      const wheelTarget = area === 'header' ? dialog.querySelector('header')!
        : area === 'backdrop' ? dialog.parentElement! : list
      const event = new WheelEvent('wheel', { deltaY: area === 'top edge' ? -40 : 40, bubbles: true, cancelable: true })
      const initialListTop = list.scrollTop
      wheelTarget.dispatchEvent(event)
      expect(settingsPanel.scrollTop).toBe(100)
      expect(event.defaultPrevented).toBe(true)
      if (area === 'list') expect(list.scrollTop).toBeGreaterThan(initialListTop)
      else expect(list.scrollTop).toBe(initialListTop)
    } finally {
      await unmount(inner)
      await unmount(outer)
      target.remove()
    }
  },
)

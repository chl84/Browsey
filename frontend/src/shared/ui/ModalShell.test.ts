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

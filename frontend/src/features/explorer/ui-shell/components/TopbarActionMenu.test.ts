import { mount, tick, unmount } from 'svelte'
import { afterEach, expect, it, vi } from 'vitest'
import TopbarActionMenu from './TopbarActionMenu.svelte'

const components: ReturnType<typeof mount>[] = []
afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})
it('focuses a real menu control so Escape reaches its close handler', async () => {
  const onClose = vi.fn()
  components.push(mount(TopbarActionMenu, { target: document.body, props: { open: true, onClose } }))
  await tick(); await tick()
  const first = document.querySelector<HTMLButtonElement>('[role="menuitem"]')!
  expect(document.activeElement).toBe(first)
  first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  expect(onClose).toHaveBeenCalledOnce()
})
it('returns focus to the opener before an action opens its dialog', async () => {
  const opener = document.createElement('button'); document.body.append(opener); opener.focus()
  let focusAtAction: Element | null = null
  const onSelect = vi.fn(() => { focusAtAction = document.activeElement })
  components.push(mount(TopbarActionMenu, { target: document.body, props: { open: true, onSelect } }))
  await tick(); await tick()
  document.querySelector<HTMLButtonElement>('[role="menuitem"]')!.click()
  expect(onSelect).toHaveBeenCalledWith('open-settings')
  expect(focusAtAction).toBe(opener)
})

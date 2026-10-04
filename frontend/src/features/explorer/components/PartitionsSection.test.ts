import { mount, tick, unmount } from 'svelte'
import { afterEach, expect, it, vi } from 'vitest'
import PartitionsSection from './PartitionsSection.svelte'

const components: ReturnType<typeof mount>[] = []

afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

it('shows capacity next to the name without changing navigation or USB controls', async () => {
  const onSelect = vi.fn()
  components.push(mount(PartitionsSection, {
    target: document.body,
    props: {
      onSelect,
      partitions: [
        { label: '/', path: '/', sizeBytes: 512_000_000_000 },
        { label: 'USB', path: '/media/USB', removable: true, sizeBytes: 32_000_000_000 },
        { label: 'Offline', path: 'usb-volume:///dev/sdb1', removable: true, sizeBytes: 16_000_000_000 },
      ],
    },
  }))
  await tick()
  const buttons = document.querySelectorAll<HTMLButtonElement>('button.nav')
  expect(buttons[0].querySelector('.capacity')?.textContent?.trim()).toBe('512 GB')
  expect(buttons[1].querySelector('.capacity')?.textContent?.trim()).toBe('32 GB')
  expect(buttons[2].querySelector('.capacity')?.textContent?.trim()).toBe('16 GB')
  expect(buttons[2].textContent).toContain('(not mounted)')
  buttons[1].click()
  expect(onSelect).toHaveBeenCalledExactlyOnceWith('/media/USB')
  expect(document.querySelectorAll('[aria-label="USB actions"]')).toHaveLength(2)
  expect(document.querySelectorAll('[aria-label="Eject"]')).toHaveLength(1)
})

it('does not invent capacity for phone, network or unavailable volumes', async () => {
  components.push(mount(PartitionsSection, {
    target: document.body,
    props: {
      partitions: [
        { label: 'Phone', path: 'mtp://phone', fs: 'mtp', removable: true },
        { label: 'NAS', path: 'smb://nas/share' },
        { label: 'Unknown', path: '/unknown', sizeBytes: null },
        { label: 'Empty', path: '/empty', sizeBytes: 0 },
      ],
    },
  }))
  await tick()
  expect(document.querySelector('.capacity')).toBeNull()
  expect(document.querySelector('[aria-label="Phone actions"]')).not.toBeNull()
})

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

it('puts each name word on its own line while preserving full names and mount status', async () => {
  vi.useFakeTimers()
  try {
    const onSelect = vi.fn()
    const phone = { label: 'SAMSUNG Android', path: '/mock/Phone', fs: 'mtp', removable: true }
    const disk = { label: '  Backup\tUSB  Disk  ', path: '/media/backup', sizeBytes: 32_000_000_000 }
    const offline = { label: 'VeryLongPartitionName', path: 'usb-volume:///dev/sdb1', removable: true }
    components.push(mount(PartitionsSection, {
      target: document.body, props: { partitions: [phone, disk, offline], onSelect },
    }))
    await tick()
    const buttons = document.querySelectorAll<HTMLButtonElement>('button.nav')
    const words = (button: HTMLElement) => Array.from(button.querySelectorAll('.nav-word'), word => word.textContent)
    expect(words(buttons[0])).toEqual(['SAMSUNG', 'Android'])
    expect(words(buttons[1])).toEqual(['Backup', 'USB', 'Disk'])
    expect(words(buttons[2])).toEqual(['VeryLongPartitionName', '(not mounted)'])
    expect(buttons[0].getAttribute('aria-label')).toBe('SAMSUNG Android')
    expect(buttons[2].getAttribute('aria-label')).toBe('VeryLongPartitionName (not mounted)')
    expect(buttons[1].querySelector('.capacity')?.textContent?.trim()).toBe('32 GB')
    buttons[0].click()
    expect(onSelect).toHaveBeenCalledExactlyOnceWith(phone.path)
    buttons[0].querySelector('.nav-label')!.dispatchEvent(new MouseEvent('mouseenter'))
    await vi.advanceTimersByTimeAsync(750)
    expect(document.querySelector('.browsey-tooltip')?.textContent).toBe(phone.label)
  } finally {
    vi.useRealTimers()
  }
})

it.each(['contextmenu', 'ContextMenu', 'Shift+F10'])('opens Properties for a fixed volume using %s without offering format', async (action) => {
  const part = { label: '/', path: '/', fs: 'btrfs', removable: false }
  const onProperties = vi.fn()
  components.push(mount(PartitionsSection, {
    target: document.body, props: { partitions: [part] },
    events: { properties: onProperties },
  }))
  await tick()
  const button = document.querySelector<HTMLButtonElement>('.nav')!
  if (action === 'contextmenu') {
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    button.dispatchEvent(event)
    expect(event.defaultPrevented).toBe(true)
  } else {
    button.dispatchEvent(new KeyboardEvent('keydown', {
      key: action === 'Shift+F10' ? 'F10' : action,
      shiftKey: action === 'Shift+F10', bubbles: true, cancelable: true,
    }))
  }
  await tick()
  const items = document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')
  expect(items).toHaveLength(1)
  expect(items[0].textContent).toContain('Properties')
  items[0].click()
  expect(onProperties).toHaveBeenCalledOnce()
  expect(onProperties.mock.calls[0][0].detail.part).toEqual(part)
})

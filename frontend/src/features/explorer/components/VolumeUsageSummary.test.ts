import { mount, tick, unmount } from 'svelte'
import { afterEach, expect, it } from 'vitest'
import VolumeUsageSummary from './VolumeUsageSummary.svelte'
import PropertiesModal from './PropertiesModal.svelte'
import type { VolumeUsageState } from '../modals/propertiesModal'

const components: ReturnType<typeof mount>[] = []
const data = { totalBytes: 32_000_000_000, usedBytes: 8_000_000_000, freeBytes: 23_000_000_000, reservedBytes: 1_000_000_000 }
const ready: VolumeUsageState = { data, loading: false, error: null }

afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

it('shows compact total/used/free statistics with an accessible usage meter and reserved-space explanation', async () => {
  components.push(mount(VolumeUsageSummary, { target: document.body, props: { usage: ready } }))
  await tick()
  const meter = document.querySelector('[role="meter"]')!
  expect(meter.getAttribute('aria-valuenow')).toBe('25')
  expect(meter.getAttribute('aria-valuetext')).toBe('8 GB used of 32 GB')
  expect(document.querySelector('.legend')?.textContent).toContain('32 GB total')
  expect(document.querySelector('.legend')?.textContent).toContain('8 GB used')
  expect(document.querySelector('.legend')?.textContent).toContain('23 GB free')
  const descriptionId = document.querySelector('.volume-usage')?.getAttribute('aria-describedby')
  expect(document.getElementById(descriptionId!)?.textContent).toContain('1 GB is reserved')
})

it.each([0, 32_000_000_000])('displays zero used/free bytes correctly (used=%s)', async (usedBytes) => {
  components.push(mount(VolumeUsageSummary, {
    target: document.body,
    props: { usage: { ...ready, data: { ...data, usedBytes, freeBytes: data.totalBytes - usedBytes, reservedBytes: 0 } } },
  }))
  await tick()
  expect(document.querySelector('.legend')?.textContent).toContain(usedBytes === 0 ? '0 GB used' : '0 GB free')
  expect(document.querySelector('[role="meter"]')?.getAttribute('aria-valuenow')).toBe(usedBytes === 0 ? '0' : '100')
})

it('shows loading without misleading zero values', async () => {
  components.push(mount(VolumeUsageSummary, {
    target: document.body, props: { usage: { data: null, loading: true, error: null } },
  }))
  await tick()
  expect(document.querySelector('.volume-usage')?.getAttribute('aria-busy')).toBe('true')
  expect(document.querySelector('[role="status"]')?.textContent).toContain('Reading storage usage')
  expect(document.querySelector('[role="meter"]')).toBeNull()
})

it('shows known capacity and an explicit mount hint for an offline device', async () => {
  components.push(mount(VolumeUsageSummary, {
    target: document.body, props: { capacity: data.totalBytes, unmounted: true },
  }))
  await tick()
  expect(document.body.textContent).toContain('32 GB total')
  expect(document.body.textContent).toContain('after mounting')
  expect(document.querySelector('[role="meter"]')).toBeNull()
})

it('shows an unobtrusive read error rather than a fabricated chart', async () => {
  components.push(mount(VolumeUsageSummary, {
    target: document.body, props: { usage: { data: null, loading: false, error: 'Could not read storage usage.' } },
  }))
  await tick()
  expect(document.querySelector('[role="status"]')?.textContent).toContain('Could not read storage usage.')
  expect(document.querySelector('[role="meter"]')).toBeNull()
})

it('uses the shared Properties modal Basic tab for a fixed volume and hides file-only fields', async () => {
  const partition = { label: '/', path: '/', fs: 'btrfs', removable: false }
  components.push(mount(PropertiesModal, {
    target: document.body,
    props: { open: true, partition, volumeUsage: ready, entry: { name: '/', path: '/', kind: 'dir', iconId: 0 } },
  }))
  await tick()
  expect(document.querySelector('[role="dialog"]')).not.toBeNull()
  expect(document.querySelector('.basic-rows')?.textContent).toContain('Volume')
  expect(document.querySelector('.basic-rows')?.textContent).not.toContain('Hidden')
  expect(document.querySelector('[role="meter"]')).not.toBeNull()
  expect(Array.from(document.querySelectorAll('.tabs button')).map(el => el.textContent?.trim())).toEqual(['Basic', 'Ownership', 'Permissions'])
})

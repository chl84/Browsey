import { mount, tick, unmount } from 'svelte'
import { afterEach, expect, it, vi } from 'vitest'
import PropertiesModal from './PropertiesModal.svelte'
import type { PermissionsState } from '../modals/propertiesModal'

const components: ReturnType<typeof mount>[] = []
const permissions: PermissionsState = {
  restriction: 'write_protection', accessSupported: true, ownershipSupported: false,
  ownerName: 'user', groupName: 'user',
  owner: { read: true, write: true, exec: true },
  group: { read: true, write: false, exec: true },
  other: { read: true, write: false, exec: true },
}

afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

const openPermissions = async (value: PermissionsState, onToggleAccess = vi.fn()) => {
  components.push(mount(PropertiesModal, {
    target: document.body,
    props: { open: true, entry: { path: '/media/USB/file.pdf', name: 'file.pdf', kind: 'file', iconId: 0 }, permissions: value, onToggleAccess },
  }))
  await tick()
  const tab = Array.from(document.querySelectorAll<HTMLButtonElement>('.tabs button')).find(button => button.textContent === 'Permissions')!
  tab.click()
  await tick()
  return onToggleAccess
}

it('disables unsupported FAT read/execute and per-group writes while preserving file write protection', async () => {
  const toggle = await openPermissions(permissions)
  const boxes = Array.from(document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'))
  expect(boxes).toHaveLength(9)
  expect(boxes.filter(box => !box.disabled).map(box => box.getAttribute('aria-label'))).toEqual(['Owner write permission'])
  expect(document.querySelector('.permission-hint')?.textContent).toBe('Only write protection is supported; it applies to everyone.')
  const write = boxes.find(box => !box.disabled)!
  write.click()
  expect(toggle).toHaveBeenCalledWith('owner', 'write', false)
})

it('disables all edits for FAT directories and masks that cannot represent write protection', async () => {
  await openPermissions({ ...permissions, restriction: 'mount_managed' })
  expect(Array.from(document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).every(box => box.disabled)).toBe(true)
  expect(document.querySelector('.permission-hint')?.textContent).toBe('Permissions are controlled by mount options.')
})

it('retains all permission controls on ordinary Unix filesystems', async () => {
  await openPermissions({ ...permissions, restriction: null, ownershipSupported: true })
  expect(document.querySelectorAll('input[type="checkbox"]:not(:disabled)')).toHaveLength(9)
  expect(document.querySelector('.permission-hint')).toBeNull()
})

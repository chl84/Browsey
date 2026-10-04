import { mount, tick, unmount, type ComponentProps } from 'svelte'
import { afterEach, expect, it, vi } from 'vitest'
import FormatUsbModal from './FormatUsbModal.svelte'
import type { UsbFormatInfo } from '../services/drives.service'

const components: ReturnType<typeof mount>[] = []
const info: UsbFormatInfo = {
  device: '/dev/test-only', model: 'Test USB', sizeBytes: 32_000_000_000,
  filesystems: [
    { id: 'exfat', label: 'exFAT', description: 'Compatible everywhere', available: true, requiredTool: 'mkfs.exfat', labelMaxLength: 11 },
    { id: 'ntfs', label: 'NTFS', description: 'Windows filesystem', available: true, requiredTool: 'mkntfs', labelMaxLength: 128 },
  ],
}

afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

const open = async (props: Partial<ComponentProps<FormatUsbModal>> = {}) => {
  const onConfirm = vi.fn()
  components.push(mount(FormatUsbModal, { target: document.body, props: { open: true, info, onConfirm, ...props } }))
  await tick()
  return onConfirm
}
const eraseButton = () => document.querySelector<HTMLButtonElement>('button.danger')!

it('offers NTFS with its backend label limit and retains the whole-drive warning', async () => {
  const confirm = await open({ filesystem: 'ntfs', label: 'Windows_backup_2026' })
  expect(document.querySelector<HTMLInputElement>('#usb-volume-label')!.maxLength).toBe(128)
  expect(document.querySelector<HTMLOptionElement>('option[value="ntfs"]')!.disabled).toBe(false)
  expect(document.body.textContent).toContain('permanently erases all data')
  expect(document.body.textContent).toContain('replaces its partition layout')
  eraseButton().click()
  expect(confirm).toHaveBeenCalledOnce()
})

it('keeps missing tools visible but cannot format an unavailable selection', async () => {
  const confirm = await open({ filesystem: 'ntfs', info: { ...info, filesystems: info.filesystems.map(option => ({ ...option, available: option.id !== 'ntfs' })) } })
  expect(document.querySelector<HTMLOptionElement>('option[value="ntfs"]')!.disabled).toBe(true)
  expect(document.body.textContent).toContain('NTFS requires mkntfs')
  expect(eraseButton().disabled).toBe(true)
  eraseButton().click()
  expect(confirm).not.toHaveBeenCalled()
})

it('preserves a long NTFS label when switching formats and requires correction', async () => {
  const confirm = await open({ filesystem: 'ntfs', label: 'Windows_backup_2026' })
  const select = document.querySelector<HTMLSelectElement>('#usb-filesystem')!
  select.value = 'exfat'
  select.dispatchEvent(new Event('change', { bubbles: true }))
  await tick()
  const input = document.querySelector<HTMLInputElement>('#usb-volume-label')!
  expect(input.value).toBe('Windows_backup_2026')
  expect(input.maxLength).toBe(11)
  expect(input.getAttribute('aria-invalid')).toBe('true')
  expect(document.querySelector('[role="alert"]')?.textContent).toContain('up to 11 ASCII')
  expect(eraseButton().disabled).toBe(true)
  input.value = 'BACKUP'
  input.dispatchEvent(new Event('input', { bubbles: true }))
  await tick()
  expect(eraseButton().disabled).toBe(false)
  eraseButton().click()
  expect(confirm).toHaveBeenCalledOnce()
})

it.each(['bad/name', 'bad\nname', 'følsom', 'A'.repeat(129)])('rejects invalid NTFS label %s', async label => {
  const confirm = await open({ filesystem: 'ntfs', label })
  expect(eraseButton().disabled).toBe(true)
  eraseButton().click()
  expect(confirm).not.toHaveBeenCalled()
})

it('does not offer formatting when no filesystem tools are available', async () => {
  await open({ info: { ...info, filesystems: info.filesystems.map(option => ({ ...option, available: false })) } })
  expect(document.querySelector<HTMLSelectElement>('#usb-filesystem')!.disabled).toBe(true)
  expect(eraseButton().disabled).toBe(true)
})

it('retains shared progress and disables controls while formatting NTFS', async () => {
  await open({ filesystem: 'ntfs', busy: true, progress: { phase: 'Creating filesystem', percent: 42 } })
  expect(document.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('42')
  expect(document.querySelector<HTMLInputElement>('#usb-volume-label')!.disabled).toBe(true)
  expect(document.querySelector<HTMLSelectElement>('#usb-filesystem')!.disabled).toBe(true)
  expect(document.querySelector<HTMLButtonElement>('button[data-cancel]')!.disabled).toBe(true)
  expect(eraseButton().disabled).toBe(true)
})

it('reports mounted NTFS success without offering another erase', async () => {
  await open({ result: { device: '/dev/test-only', filesystem: 'NTFS', label: 'BACKUP', sizeBytes: 32_000_000_000, mountPath: '/media/BACKUP' } })
  expect(document.body.textContent).toContain('BACKUP is formatted as NTFS and ready to use')
  expect(document.body.textContent).toContain('/media/BACKUP')
  expect(document.querySelector('button.danger')).toBeNull()
})

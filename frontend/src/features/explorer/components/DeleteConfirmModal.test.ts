import { mount, tick, unmount } from 'svelte'
import { afterEach, expect, it, vi } from 'vitest'
import DeleteConfirmModal from './DeleteConfirmModal.svelte'

const components: ReturnType<typeof mount>[] = []
afterEach(async () => {
  for (const component of components.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

it('warns about direct network deletion using the shared dialog and retains Cancel', async () => {
  const onConfirm = vi.fn()
  const onCancel = vi.fn()
  components.push(mount(DeleteConfirmModal, { target: document.body, props: { open: true, mode: 'network', targetLabel: '/mnt/share/large.bin', onConfirm, onCancel } }))
  await tick()
  expect(document.querySelector('[role="dialog"]')).not.toBeNull()
  expect(document.body.textContent).toContain('without a local backup')
  expect(document.body.textContent).toContain('Browsey cannot undo this')
  expect(onConfirm).not.toHaveBeenCalled()
  document.querySelector<HTMLButtonElement>('button.secondary')!.click()
  expect(onCancel).toHaveBeenCalledOnce()
  expect(onConfirm).not.toHaveBeenCalled()
})

it('explains unsupported remote trash without claiming that every target will be permanently deleted', async () => {
  const onConfirm = vi.fn()
  components.push(mount(DeleteConfirmModal, { target: document.body, props: { open: true, mode: 'network-trash', targetLabel: '2 items', onConfirm } }))
  await tick()
  expect(document.body.textContent).toContain('Other items will be moved to trash where supported')
  expect(document.body.textContent).toContain('will not download a backup')
  document.querySelector<HTMLButtonElement>('button.danger')!.click()
  expect(onConfirm).toHaveBeenCalledOnce()
})

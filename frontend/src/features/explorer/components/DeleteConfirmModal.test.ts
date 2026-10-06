import { mount, tick, unmount } from 'svelte'
import { afterEach, expect, it, vi } from 'vitest'
import DeleteConfirmModal from './DeleteConfirmModal.svelte'

const components: ReturnType<typeof mount>[] = []
it('warns that purging the Wastebasket cannot be undone and allows cancellation', async () => {
  const onCancel = vi.fn()
  const onConfirm = vi.fn()
  components.push(mount(DeleteConfirmModal, { target: document.body, props: { open: true, mode: 'trash', targetLabel: 'generated.txt', onCancel, onConfirm } }))
  await tick()
  expect(document.body.textContent).toContain('permanently removed from the Wastebasket')
  expect(document.body.textContent).toContain('This cannot be undone')
  expect(document.body.textContent).not.toContain('supported local deletions')
  document.querySelector<HTMLButtonElement>('button.secondary')!.click()
  expect(onCancel).toHaveBeenCalledOnce()
  expect(onConfirm).not.toHaveBeenCalled()
})
it('distinguishes session-local Undo from irreversible remote deletion', async () => {
  components.push(mount(DeleteConfirmModal, { target: document.body, props: { open: true, targetLabel: 'generated.txt' } }))
  await tick()
  expect(document.body.textContent).toContain('supported local deletions')
  expect(document.body.textContent).toContain('while this session is running')
  expect(document.body.textContent).toContain('Cloud and network deletions cannot be undone')
})
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

import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const { listenMock, eventHandlers } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  eventHandlers: new Map<string, (event: { payload: unknown }) => void>(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('../services/activity.service', () => ({
  cancelTask: vi.fn(async () => {}),
}))

import { createActivity } from './createActivity'
import { cancelTask } from '../services/activity.service'

describe('createActivity', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    eventHandlers.clear()
    listenMock.mockImplementation(async (eventName: string, handler: (event: { payload: unknown }) => void) => {
      eventHandlers.set(eventName, handler)
      return async () => {
        eventHandlers.delete(eventName)
      }
    })
  })

  it('formats byte progress details for active transfers', async () => {
    const activityApi = createActivity()

    await activityApi.start('Copying…', 'copy-progress-1')
    const handler = eventHandlers.get('copy-progress-1')
    expect(handler).toBeTypeOf('function')

    handler?.({ payload: { bytes: 1536, total: 4096, finished: false } })

    expect(get(activityApi.activity)).toEqual({
      label: 'Copying…',
      detail: '1.50 KB / 4.00 KB',
      percent: 38,
      cancel: null,
      cancelling: false,
    })
  })

  it('keeps a staged cloud operation cancellable until the command reply, not a child finish', async () => {
    const activityApi = createActivity()
    const cancel = vi.fn()
    await activityApi.start('Compressing…', 'staged', cancel, { completeOnReply: true })
    const handler = eventHandlers.get('staged')
    handler?.({ payload: { bytes: 8, total: 8, finished: true } })
    expect(get(activityApi.activity)).toMatchObject({ percent: null, cancel })
    expect(activityApi.hasHideTimer()).toBe(false)
    handler?.({ payload: { bytes: 0, total: 0, finished: false, phase: 'Uploading archive…' } })
    expect(get(activityApi.activity)).toMatchObject({ label: 'Uploading archive…', percent: null, cancel })
    await activityApi.requestCancel('staged')
    handler?.({ payload: { bytes: 8, total: 8, finished: true } })
    expect(get(activityApi.activity)).toMatchObject({ label: 'Cancelling…', cancel: null })
    expect(activityApi.hasHideTimer()).toBe(false)
    await activityApi.cleanup()
  })

  it('keeps cancellation pending when a preparing or finishing task is not registered', async () => {
    const onError = vi.fn()
    const activityApi = createActivity({ onError })
    await activityApi.start('Copying…', 'preparing', () => {}, { completeOnReply: true })
    vi.mocked(cancelTask).mockRejectedValueOnce({ code: 'task_not_found', message: 'Task not found' })
    await activityApi.requestCancel('preparing')
    expect(get(activityApi.activity)).toMatchObject({ label: 'Cancelling…', cancelling: true })
    expect(onError).not.toHaveBeenCalled()
    expect(eventHandlers.has('preparing')).toBe(true)
    await activityApi.cleanup()
    expect(eventHandlers.has('preparing')).toBe(false)
  })

  it('still reports an unexpected cancellation failure and releases its listener', async () => {
    const onError = vi.fn()
    const activityApi = createActivity({ onError })
    await activityApi.start('Copying…', 'broken', () => {})
    vi.mocked(cancelTask).mockRejectedValueOnce({ code: 'registry_failed', message: 'Registry unavailable' })
    await activityApi.requestCancel('broken')
    expect(get(activityApi.activity)).toBe(null)
    expect(onError).toHaveBeenCalledWith('Cancel failed: Registry unavailable')
    expect(eventHandlers.has('broken')).toBe(false)
  })

  it('clears byte details while cancelling', async () => {
    const activityApi = createActivity()

    await activityApi.start('Uploading…', 'upload-progress-1', () => {})
    await activityApi.requestCancel('upload-progress-1')

    const handler = eventHandlers.get('upload-progress-1')
    handler?.({ payload: { bytes: 1024, total: 2048, finished: false } })

    expect(get(activityApi.activity)).toEqual({
      label: 'Cancelling…',
      detail: null,
      percent: 50,
      cancel: null,
      cancelling: true,
    })
  })

  it.each(['delete', 'trash'])('formats %s progress as items, not bytes', async (operation) => {
    const activityApi = createActivity()
    const eventName = `${operation}-progress-1`
    await activityApi.start('Deleting…', eventName)
    const handler = eventHandlers.get(eventName)

    handler?.({ payload: { unit: 'items', items: 1, total: 3, finished: false } })
    expect(get(activityApi.activity)).toMatchObject({
      label: 'Deleting…',
      detail: '1 / 3 items',
      percent: 33,
    })
    await activityApi.cleanup()
  })

  it('keeps item units when finalizing deletion of a single photo', async () => {
    const activityApi = createActivity()
    await activityApi.start('Deleting…', 'delete-progress-1')
    eventHandlers.get('delete-progress-1')?.({
      payload: { unit: 'items', items: 1, total: 1, finished: true },
    })

    expect(get(activityApi.activity)).toMatchObject({
      label: 'Finalizing…',
      detail: '1 / 1 item',
      percent: 100,
      cancel: null,
    })
    expect(activityApi.hasHideTimer()).toBe(true)
    await activityApi.cleanup()
  })

  it('does not format large item counts as kilobytes', async () => {
    const activityApi = createActivity()
    await activityApi.start('Deleting…', 'delete-progress-1')
    eventHandlers.get('delete-progress-1')?.({
      payload: { unit: 'items', items: 1024, total: 2048 },
    })

    expect(get(activityApi.activity)).toMatchObject({ detail: '1024 / 2048 items', percent: 50 })
    await activityApi.cleanup()
  })

  it('keeps empty deletion progress indeterminate without a size label', async () => {
    const activityApi = createActivity()
    await activityApi.start('Deleting…', 'delete-progress-1')
    eventHandlers.get('delete-progress-1')?.({
      payload: { unit: 'items', items: 0, total: 0, finished: true },
    })

    expect(get(activityApi.activity)).toMatchObject({ detail: null, percent: null })
    await activityApi.cleanup()
  })

  it('preserves cancellation handling for item progress', async () => {
    const activityApi = createActivity()
    await activityApi.start('Deleting…', 'delete-progress-1', () => {})
    await activityApi.requestCancel('delete-progress-1')
    eventHandlers.get('delete-progress-1')?.({
      payload: { unit: 'items', items: 1, total: 2, finished: false },
    })

    expect(get(activityApi.activity)).toMatchObject({
      label: 'Cancelling…', detail: null, percent: 50, cancel: null, cancelling: true,
    })
    await activityApi.cleanup()
  })
})

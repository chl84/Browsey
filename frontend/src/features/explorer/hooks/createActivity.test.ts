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

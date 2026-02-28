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
})

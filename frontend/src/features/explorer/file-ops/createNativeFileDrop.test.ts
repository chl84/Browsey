import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { createNativeFileDrop } from './createNativeFileDrop'

let dragDropCallback: ((event: { payload: unknown }) => void) | null = null
const unlistenMock = vi.fn(async () => {})
const onDragDropEventMock = vi.fn(async (callback: (event: { payload: unknown }) => void) => {
  dragDropCallback = callback
  return unlistenMock
})

vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: vi.fn(async () => ({
    onDragDropEvent: onDragDropEventMock,
  })),
}))

describe('createNativeFileDrop', () => {
  beforeEach(() => {
    dragDropCallback = null
    unlistenMock.mockClear()
    onDragDropEventMock.mockClear()
  })

  it('updates hover state and forwards dropped paths', async () => {
    const onDrop = vi.fn()
    const drop = createNativeFileDrop({ onDrop })
    await drop.start()

    expect(onDragDropEventMock).toHaveBeenCalledTimes(1)
    expect(dragDropCallback).not.toBeNull()

    dragDropCallback?.({
      payload: { type: 'over', position: { x: 12, y: 24 } },
    })
    expect(get(drop.hovering)).toBe(true)
    expect(get(drop.position)).toEqual({ x: 12, y: 24 })

    dragDropCallback?.({
      payload: { type: 'drop', paths: ['/tmp/a.txt', '/tmp/b.txt'] },
    })
    expect(get(drop.hovering)).toBe(false)
    expect(get(drop.position)).toBeNull()
    expect(onDrop).toHaveBeenCalledWith(['/tmp/a.txt', '/tmp/b.txt'])

    await drop.stop()
    expect(unlistenMock).toHaveBeenCalledTimes(1)
  })
})

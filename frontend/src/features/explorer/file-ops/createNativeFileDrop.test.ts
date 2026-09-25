import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { get } from 'svelte/store'

const listen = vi.fn()
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ onDragDropEvent: listen }) }))
import { createNativeFileDrop } from './createNativeFileDrop'

describe('native drop lifecycle and coordinates', () => {
  beforeEach(() => { vi.clearAllMocks() })
  it('handles enter/over/drop in CSS pixels and keeps source paths during hover', async () => {
    const ratio = vi.spyOn(window, 'devicePixelRatio', 'get').mockReturnValue(2)
    let handler!: (event: { payload: DragDropEvent }) => void
    const unlisten = vi.fn()
    listen.mockImplementation(callback => { handler = callback; return Promise.resolve(unlisten) })
    const options = { onHover: vi.fn(), onDrop: vi.fn(), onLeave: vi.fn() }
    const drop = createNativeFileDrop(options)
    try {
      await drop.start()
      const send = (payload: unknown) => handler({ payload: payload as DragDropEvent })
      send({ type: 'enter', paths: ['/source'], position: { x: 200, y: 100 } })
      expect(options.onHover).toHaveBeenLastCalledWith(['/source'], { x: 100, y: 50 })
      send({ type: 'over', position: { x: 220, y: 120 } })
      expect(options.onHover).toHaveBeenLastCalledWith(['/source'], { x: 110, y: 60 })
      send({ type: 'drop', paths: ['/source'], position: { x: 240, y: 140 } })
      expect(options.onDrop).toHaveBeenCalledWith(['/source'], { x: 120, y: 70 })
      expect(get(drop.hovering)).toBe(false)
      expect(get(drop.position)).toBeNull()
      send({ type: 'leave' })
      expect(options.onLeave).toHaveBeenCalledTimes(1)
      await drop.stop()
      expect(unlisten).toHaveBeenCalledTimes(1)
    } finally { ratio.mockRestore() }
  })
  it('deduplicates concurrent starts and retires a listener resolved after stop', async () => {
    let resolve!: (fn: () => void) => void
    listen.mockReturnValue(new Promise(r => { resolve = r }))
    const drop = createNativeFileDrop()
    const first = drop.start()
    const second = drop.start()
    expect(first).toBe(second)
    await Promise.resolve()
    await drop.stop()
    const unlisten = vi.fn()
    resolve(unlisten)
    await first
    expect(listen).toHaveBeenCalledTimes(1)
    expect(unlisten).toHaveBeenCalledTimes(1)
  })
  it('reports rejected drop callbacks without leaking an unhandled rejection', async () => {
    let handler!: (event: { payload: unknown }) => void
    listen.mockImplementation(callback => { handler = callback; return Promise.resolve(vi.fn()) })
    const onError = vi.fn()
    const drop = createNativeFileDrop({ onDrop: async () => { throw new Error('failure') }, onError })
    await drop.start()
    handler({ payload: { type: 'drop', paths: ['/source'], position: { x: 1, y: 2 } } })
    await Promise.resolve()
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'failure' }))
    await drop.stop()
  })
})

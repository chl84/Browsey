import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get, writable } from 'svelte/store'

const setClipboardPathsStateMock = vi.fn()
const setClipboardCmdMock = vi.fn()
const resolveDropClipboardModeMock = vi.fn()
const startNativeFileDragMock = vi.fn()
let onNativeDrop: (paths: string[], point: { x: number; y: number }) => Promise<void>
let onNativeHover: (paths: string[], point: { x: number; y: number }) => void
let onNativeLeave: () => void

vi.mock('svelte', async () => {
  const actual = await vi.importActual<typeof import('svelte')>('svelte')
  return {
    ...actual,
    onDestroy: () => {},
  }
})

vi.mock('./createNativeFileDrop', () => ({
  createNativeFileDrop: vi.fn((options: { onDrop: typeof onNativeDrop; onHover: typeof onNativeHover; onLeave: () => void }) => {
    onNativeDrop = options.onDrop
    onNativeHover = options.onHover
    onNativeLeave = options.onLeave
    return {
    hovering: writable(false),
    position: writable(null),
    start: vi.fn(async () => {}),
    stop: vi.fn(async () => {}),
    }
  }),
}))

vi.mock('./clipboard.store', () => ({
  setClipboardPathsState: (...args: unknown[]) => setClipboardPathsStateMock(...args),
}))

vi.mock('../services/clipboard.service', () => ({
  resolveDropClipboardMode: (...args: unknown[]) => resolveDropClipboardModeMock(...args),
  setClipboardCmd: (...args: unknown[]) => setClipboardCmdMock(...args),
}))

vi.mock('../services/nativeDrag.service', () => ({
  startNativeFileDrag: (...args: unknown[]) => startNativeFileDragMock(...args),
}))

import { useExplorerDragDrop } from './useExplorerDragDrop'

const createDataTransfer = () =>
  ({
    effectAllowed: 'copyMove',
    dropEffect: 'none',
    setData: vi.fn(),
    setDragImage: vi.fn(),
  }) as unknown as DataTransfer

const createDragEvent = (overrides: Partial<DragEvent> = {}) =>
  ({
    clientX: 12,
    clientY: 24,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    dataTransfer: createDataTransfer(),
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    ...overrides,
  }) as unknown as DragEvent

describe('useExplorerDragDrop bookmark drop handlers', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.clearAllMocks()
    resolveDropClipboardModeMock.mockResolvedValue('copy')
    setClipboardCmdMock.mockResolvedValue(undefined)
    startNativeFileDragMock.mockResolvedValue(true)
  })
  afterEach(() => { vi.clearAllTimers(); vi.useRealTimers(); document.body.innerHTML = '' })

  it('drops onto bookmark paths through the same paste flow as breadcrumbs', async () => {
    const handlePasteOrMove = vi.fn(async () => true)
    const dragDrop = useExplorerDragDrop({
      currentView: () => 'dir',
      currentPath: () => '/tmp/current',
      getSelectedSet: () => new Set(['/tmp/source.txt']),
      loadDir: vi.fn(async () => {}),
      isBlocked: () => false, isSearchActive: () => false,
      handlePasteOrMove,
      showToast: vi.fn(),
    })

    dragDrop.handleRowDragStart(
      { path: '/tmp/source.txt', kind: 'file', name: 'source.txt' } as never,
      createDragEvent(),
    )
    dragDrop.handleBookmarkDragOver('/tmp/bookmark-target', createDragEvent())

    expect(get(dragDrop.dragState).target).toBe('/tmp/bookmark-target')

    await dragDrop.handleBookmarkDrop('/tmp/bookmark-target', createDragEvent())

    expect(resolveDropClipboardModeMock).toHaveBeenCalledWith(
      ['/tmp/source.txt'],
      '/tmp/bookmark-target',
      false,
    )
    expect(setClipboardPathsStateMock).not.toHaveBeenCalled()
    expect(setClipboardCmdMock).not.toHaveBeenCalled()
    expect(handlePasteOrMove).toHaveBeenCalledWith('/tmp/bookmark-target', {
      paths: ['/tmp/source.txt'], mode: 'copy',
    })
    expect(get(dragDrop.dragState).dragging).toBe(false)
    expect(get(dragDrop.dragState).target).toBeNull()
  })

  it.each(['/tmp/dest', 'rclone://work/dest'])('passes native drops explicitly without replacing the clipboard (%s)', async (dest) => {
    const handlePasteOrMove = vi.fn(async () => true)
    useExplorerDragDrop({
      currentView: () => 'dir', currentPath: () => dest,
      getSelectedSet: () => new Set(), loadDir: vi.fn(),
      isBlocked: () => false, isSearchActive: () => false, showToast: vi.fn(), handlePasteOrMove,
    })
    const background = document.createElement('div')
    background.setAttribute('data-drop-background', '')
    document.body.append(background)
    document.elementFromPoint = vi.fn(() => background)
    await onNativeDrop(['/tmp/dropped.jpg'], { x: 12, y: 24 })
    expect(handlePasteOrMove).toHaveBeenCalledWith(dest, { paths: ['/tmp/dropped.jpg'], mode: 'copy' })
    expect(setClipboardPathsStateMock).not.toHaveBeenCalled()
    expect(setClipboardCmdMock).not.toHaveBeenCalled()
  })
})

describe('drop policy and destination safety', () => {
  const hooks: ReturnType<typeof useExplorerDragDrop>[] = []
  const source = { path: '/tmp/source.txt', kind: 'file', name: 'source.txt' } as never
  const setup = (overrides: Partial<Parameters<typeof useExplorerDragDrop>[0]> = {}) => {
    const deps = {
      currentView: () => 'dir' as const, currentPath: () => '/tmp',
      getSelectedSet: () => new Set<string>(), loadDir: vi.fn(async () => {}),
      isBlocked: () => false, isSearchActive: () => false,
      handlePasteOrMove: vi.fn(async () => true), showToast: vi.fn(), ...overrides,
    }
    const hook = useExplorerDragDrop(deps)
    hooks.push(hook)
    return { hook, deps }
  }
  const point = { x: 20, y: 30 }
  const target = (path?: string) => {
    const el = document.createElement('div')
    if (path === undefined) el.setAttribute('data-drop-background', '')
    else el.dataset.dropPath = path
    document.body.append(el)
    document.elementFromPoint = vi.fn(() => el)
    return el
  }
  beforeEach(() => {
    vi.useFakeTimers()
    vi.clearAllMocks()
    resolveDropClipboardModeMock.mockResolvedValue('cut')
    startNativeFileDragMock.mockResolvedValue(true)
  })
  afterEach(async () => {
    await Promise.all(hooks.splice(0).map(hook => hook.stopNativeDrop()))
    vi.clearAllTimers()
    vi.useRealTimers()
    document.body.innerHTML = ''
  })

  it.each([
    [{}, '/tmp/dest', 'cut'],
    [{ ctrlKey: true }, '/tmp/dest', 'copy'],
    [{ metaKey: true }, '/tmp/dest', 'copy'],
    [{ shiftKey: true }, '/tmp/dest', 'cut'],
    [{ ctrlKey: true, shiftKey: true }, '/tmp/dest', 'copy'],
    [{}, 'rclone://remote/dest', 'copy'],
    [{ shiftKey: true }, 'rclone://remote/dest', 'cut'],
  ] as const)('uses live event modifiers %j for %s (%s)', async (keys, dest, mode) => {
    const { hook, deps } = setup()
    hook.handleRowDragStart(source, createDragEvent({ ctrlKey: true }))
    await hook.handleBookmarkDrop(dest, createDragEvent(keys))
    expect(deps.handlePasteOrMove).toHaveBeenCalledWith(dest, { paths: ['/tmp/source.txt'], mode })
  })

  it('copies cloud sources by default without invoking the local filesystem resolver', async () => {
    const { hook, deps } = setup({ getSelectedSet: () => new Set(['/tmp/source.txt', 'rclone://remote/file']) })
    hook.handleRowDragStart({ path: 'rclone://remote/other', kind: 'file' } as never, createDragEvent())
    await hook.handleBookmarkDrop('/tmp/dest', createDragEvent())
    expect(deps.handlePasteOrMove).toHaveBeenCalledWith('/tmp/dest', { paths: ['rclone://remote/other'], mode: 'copy' })
    expect(resolveDropClipboardModeMock).not.toHaveBeenCalled()
  })

  it('copies native drops into the exact hovered folder and clears its highlight', async () => {
    const { hook, deps } = setup()
    const el = target('/tmp/actual')
    onNativeHover(['/other/picture.jpg'], point)
    expect(el.dataset.dropActive).toBe('true')
    expect(get(hook.dragState).target).toBe('/tmp/actual')
    await onNativeDrop(['/other/picture.jpg'], point)
    expect(deps.handlePasteOrMove).toHaveBeenCalledWith('/tmp/actual', { paths: ['/other/picture.jpg'], mode: 'copy' })
    expect(el.hasAttribute('data-drop-active')).toBe(false)
  })

  it.each(['', 'Recent', 'usb-volume://device', 'mtp://phone', '/tmp/source.txt/child'])('rejects invalid or recursive target %s without falling back', async path => {
    const { deps } = setup()
    target(path)
    await onNativeDrop(['/tmp/source.txt'], point)
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
    expect(deps.loadDir).not.toHaveBeenCalled()
  })

  it('blocks native and internal drops during dialogs', async () => {
    let blocked = false
    const { hook, deps } = setup({ isBlocked: () => blocked })
    target('/tmp/dest')
    hook.handleRowDragStart(source, createDragEvent())
    blocked = true
    await hook.handleBookmarkDrop('/tmp/dest', createDragEvent())
    await onNativeDrop(['/other/file'], point)
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
  })

  it('does not cancel an accepted native drop when leave arrives immediately afterwards', async () => {
    const { deps } = setup()
    target('/tmp/dest')
    onNativeHover(['/other/file'], point)
    const drop = onNativeDrop(['/other/file'], point)
    onNativeLeave()
    await drop
    expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/other/file'], mode: 'copy' })
  })

  it('rejects blank space in search but permits an explicit folder', async () => {
    const { deps } = setup({ isSearchActive: () => true })
    target()
    await onNativeDrop(['/other/file'], point)
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
    target('/tmp/result-folder')
    await onNativeDrop(['/other/file'], point)
    expect(deps.handlePasteOrMove).toHaveBeenCalledTimes(1)
  })

  it('routes an internal background drop once, even when dragend follows immediately', async () => {
    let resolve!: (mode: 'copy' | 'cut') => void
    resolveDropClipboardModeMock.mockReturnValue(new Promise(r => { resolve = r }))
    const { hook, deps } = setup()
    const el = target()
    await hook.startNativeDrop()
    hook.handleRowDragStart(source, createDragEvent())
    el.dispatchEvent(new MouseEvent('drop', { bubbles: true, cancelable: true, clientX: 20, clientY: 30 }))
    document.dispatchEvent(new Event('dragend', { bubbles: true }))
    resolve('copy')
    await vi.advanceTimersByTimeAsync(0)
    expect(deps.handlePasteOrMove).toHaveBeenCalledTimes(1)
    expect(deps.handlePasteOrMove).toHaveBeenCalledWith('/tmp', { paths: ['/tmp/source.txt'], mode: 'copy' })
  })

  it('rechecks the dialog guard after asynchronous mode resolution', async () => {
    let blocked = false
    let resolve!: (mode: 'copy' | 'cut') => void
    resolveDropClipboardModeMock.mockReturnValue(new Promise(r => { resolve = r }))
    const { hook, deps } = setup({ isBlocked: () => blocked })
    hook.handleRowDragStart(source, createDragEvent())
    const drop = hook.handleBookmarkDrop('/tmp/dest', createDragEvent())
    blocked = true
    resolve('cut')
    await drop
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
  })

  it('opens a hovered folder after the delay and cancels on leave/blur', async () => {
    const { hook, deps } = setup()
    await hook.startNativeDrop()
    target('/tmp/hover-folder')
    onNativeHover(['/other/file'], point)
    await vi.advanceTimersByTimeAsync(849)
    expect(deps.loadDir).not.toHaveBeenCalled()
    window.dispatchEvent(new Event('blur'))
    await vi.advanceTimersByTimeAsync(1000)
    expect(deps.loadDir).not.toHaveBeenCalled()
    onNativeHover(['/other/file'], point)
    await vi.advanceTimersByTimeAsync(850)
    expect(deps.loadDir).toHaveBeenCalledExactlyOnceWith('/tmp/hover-folder')
    target('/tmp/hover-folder/child')
    onNativeHover(['/other/file'], point)
    await vi.advanceTimersByTimeAsync(1000)
    expect(deps.loadDir).toHaveBeenCalledTimes(1)
    onNativeHover(['/other/file'], { x: point.x + 1, y: point.y })
    await vi.advanceTimersByTimeAsync(850)
    expect(deps.loadDir).toHaveBeenCalledTimes(2)
  })

  it('does not allow a stale preview from a previous drag to change the new target', async () => {
    let resolve!: (mode: 'copy' | 'cut') => void
    resolveDropClipboardModeMock.mockReturnValueOnce(new Promise(r => { resolve = r }))
    const { hook } = setup()
    hook.handleRowDragStart(source, createDragEvent())
    hook.handleBookmarkDragOver('/tmp/first', createDragEvent())
    hook.handleRowDragEnd()
    hook.handleRowDragStart(source, createDragEvent())
    hook.handleBookmarkDragOver('/tmp/next', createDragEvent({ ctrlKey: true }))
    resolve('cut')
    await vi.advanceTimersByTimeAsync(0)
    expect(get(hook.dragState).target).toBe('/tmp/next')
    expect(get(hook.dragAction)).toBe('copy')
  })

  it('rejects mixed selections and native cloud export with actionable feedback', () => {
    const { hook, deps } = setup({ getSelectedSet: () => new Set(['/tmp/source.txt', 'rclone://remote/file']) })
    const event = createDragEvent()
    hook.handleRowDragStart(source, event)
    expect(event.preventDefault).toHaveBeenCalled()
    hook.handleRowDragStart({ path: 'rclone://remote/other', kind: 'file' } as never, createDragEvent({ altKey: true }))
    expect(startNativeFileDragMock).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenLastCalledWith(expect.stringContaining('Download cloud files'))
  })

  it('exports local files as a native copy and handles rejected plugin promises', async () => {
    const { hook, deps } = setup()
    startNativeFileDragMock.mockRejectedValueOnce(new Error('unavailable'))
    hook.handleRowDragStart(source, createDragEvent({ altKey: true, shiftKey: true }))
    await vi.advanceTimersByTimeAsync(0)
    expect(startNativeFileDragMock).toHaveBeenCalledWith(['/tmp/source.txt'], 'copy')
    expect(deps.showToast).toHaveBeenCalledWith('Native drag failed: unavailable')
  })
})

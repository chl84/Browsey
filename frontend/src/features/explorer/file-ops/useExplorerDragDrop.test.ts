import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get, writable } from 'svelte/store'

const setClipboardPathsStateMock = vi.fn()
const setClipboardCmdMock = vi.fn()
const resolveDropClipboardModeMock = vi.fn()
const canTrashPathsMock = vi.fn()
const resolveCloudDragMock = vi.fn()
const prepareCloudDragMock = vi.fn()
vi.mock('./cloudDrag', async () => {
  const actual = await vi.importActual<typeof import('./cloudDrag')>('./cloudDrag')
  return { ...actual, resolveCloudDrag: (...args: unknown[]) => resolveCloudDragMock(...args),
    prepareCloudDrag: (...args: unknown[]) => prepareCloudDragMock(...args) }
})
vi.mock('../services/trash.service', () => ({ canTrashPaths: (...args: unknown[]) => canTrashPathsMock(...args) }))
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

import { useExplorerDragDrop } from './useExplorerDragDrop'

const createDataTransfer = () =>
  ({
    effectAllowed: 'copyMove',
    dropEffect: 'none',
    setData: vi.fn(),
    clearData: vi.fn(),
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
    canTrashPathsMock.mockResolvedValue(true)
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
    hook.handleRowDragStart(source, createDragEvent())
    await hook.handleBookmarkDrop(dest, createDragEvent(keys))
    expect(deps.handlePasteOrMove).toHaveBeenCalledWith(dest, { paths: ['/tmp/source.txt'], mode })
  })

  it('checks only Wastebasket, caches hover checks, and never opens it on hover', async () => {
    const handleTrashDrop = vi.fn(async () => true)
    const { hook, deps } = setup({ handleTrashDrop })
    const el = target('trash://')
    await hook.startNativeDrop()
    hook.handleRowDragStart(source, createDragEvent())
    expect(canTrashPathsMock).not.toHaveBeenCalled()
    hook.handleBookmarkDragOver('/tmp/dest', createDragEvent())
    expect(canTrashPathsMock).not.toHaveBeenCalled()
    resolveDropClipboardModeMock.mockClear()
    document.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: point.x, clientY: point.y }))
    await vi.advanceTimersByTimeAsync(0)
    expect(el.dataset.dropActive).toBe('true')
    expect(get(hook.dragAction)).toBe('trash')
    for (let i = 0; i < 20; i++) hook.handleBookmarkDragOver('trash://', createDragEvent())
    await vi.advanceTimersByTimeAsync(1000)
    expect(canTrashPathsMock).toHaveBeenCalledExactlyOnceWith(['/tmp/source.txt'])
    expect(deps.loadDir).not.toHaveBeenCalled()
    await hook.handleBookmarkDrop('trash://', createDragEvent({ shiftKey: true, ctrlKey: true }))
    expect(handleTrashDrop).toHaveBeenCalledExactlyOnceWith(['/tmp/source.txt'])
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
    expect(resolveDropClipboardModeMock).not.toHaveBeenCalled()
    expect(el.hasAttribute('data-drop-active')).toBe(false)
  })

  it.each(['recent', 'starred', 'dir'] as const)('uses actual source paths for Wastebasket from %s', async view => {
    const handleTrashDrop = vi.fn(async () => true)
    const { hook, deps } = setup({ currentView: () => view, isSearchActive: () => view === 'dir', handleTrashDrop })
    hook.handleRowDragStart(source, createDragEvent())
    await hook.handleBookmarkDrop('trash://', createDragEvent())
    expect(handleTrashDrop).toHaveBeenCalledExactlyOnceWith(['/tmp/source.txt'])
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
  })

  it.each([false, 'error'] as const)('rejects unsupported or failed trash checks (%s)', async result => {
    if (result === 'error') canTrashPathsMock.mockRejectedValueOnce(new Error('Disconnected'))
    else canTrashPathsMock.mockResolvedValueOnce(false)
    const handleTrashDrop = vi.fn(async () => true)
    const { hook, deps } = setup({ handleTrashDrop })
    hook.handleRowDragStart(source, createDragEvent())
    await hook.handleBookmarkDrop('trash://', createDragEvent())
    expect(handleTrashDrop).not.toHaveBeenCalled()
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith(expect.stringContaining('Nothing was deleted'))
  })

  it('does not let a late capability reply mutate a cancelled or newer drag', async () => {
    let finish!: (allowed: boolean) => void
    canTrashPathsMock.mockReturnValueOnce(new Promise<boolean>(resolve => { finish = resolve }))
    const handleTrashDrop = vi.fn(async () => true)
    const { hook } = setup({ handleTrashDrop })
    target('trash://')
    await hook.startNativeDrop()
    hook.handleRowDragStart(source, createDragEvent())
    hook.handleBookmarkDragOver('trash://', createDragEvent())
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    hook.handleRowDragStart({ path: '/tmp/new.txt', kind: 'file' } as never, createDragEvent())
    finish(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(get(hook.dragState).target).toBeNull()
    expect(handleTrashDrop).not.toHaveBeenCalled()
  })

  it('waits for capabilities at drop and blocks a duplicate native reply', async () => {
    let finish!: (allowed: boolean) => void
    canTrashPathsMock.mockReturnValueOnce(new Promise<boolean>(resolve => { finish = resolve }))
    const handleTrashDrop = vi.fn(async () => true)
    const { hook } = setup({ handleTrashDrop })
    target('trash://')
    hook.handleRowDragStart(source, createDragEvent())
    const drop = hook.handleBookmarkDrop('trash://', createDragEvent())
    await onNativeDrop(['/tmp/source.txt'], point)
    expect(handleTrashDrop).not.toHaveBeenCalled()
    finish(true)
    await drop
    expect(handleTrashDrop).toHaveBeenCalledOnce()
  })

  it('accepts native external local files and preserves a successful GTK source completion', async () => {
    const handleTrashDrop = vi.fn(async () => true)
    const { hook } = setup({ handleTrashDrop })
    const el = target('trash://')
    await hook.startNativeDrop()
    await onNativeDrop(['/outside/file.txt'], point)
    expect(handleTrashDrop).toHaveBeenCalledWith(['/outside/file.txt'])
    hook.handleRowDragStart(source, createDragEvent())
    el.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: point.x, clientY: point.y }))
    await vi.advanceTimersByTimeAsync(0)
    hook.handleRowDragEnd(createDragEvent({ dataTransfer: { dropEffect: 'move' } as DataTransfer }))
    await onNativeDrop(['/tmp/source.txt'], point)
    expect(handleTrashDrop).toHaveBeenCalledTimes(2)
  })

  it('rejects an explicitly copy-only source offer to Wastebasket', async () => {
    const handleTrashDrop = vi.fn(async () => true)
    const { hook } = setup({ handleTrashDrop })
    hook.handleRowDragStart(source, createDragEvent({ ctrlKey: true }))
    const over = createDragEvent()
    hook.handleBookmarkDragOver('trash://', over)
    expect(over.dataTransfer?.dropEffect).toBe('none')
    await hook.handleBookmarkDrop('trash://', createDragEvent())
    expect(handleTrashDrop).not.toHaveBeenCalled()
  })

  it.each([true, false] as const)('honors exact cloud entry trash capabilities (%s)', async canTrash => {
    const paths = ['rclone://Google Disk/same~id-a', 'rclone://Google Disk/same~id-b']
    const entries = paths.map(path => ({ path, name: 'same', kind: 'file', capabilities: { canTrash } })) as never[]
    const handleTrashDrop = vi.fn(async () => true)
    const { hook, deps } = setup({ getSelectedSet: () => new Set(paths), getEntries: () => entries, handleTrashDrop })
    hook.handleRowDragStart(entries[0], createDragEvent())
    await hook.handleBookmarkDrop('trash://', createDragEvent())
    if (canTrash) expect(handleTrashDrop).toHaveBeenCalledExactlyOnceWith(paths)
    else expect(handleTrashDrop).not.toHaveBeenCalled()
    expect(canTrashPathsMock).not.toHaveBeenCalled()
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
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

  it('rejects mixed selections and does not expose cloud paths as file URIs', () => {
    const { hook, deps } = setup({ getSelectedSet: () => new Set(['/tmp/source.txt', 'rclone://remote/file']) })
    const event = createDragEvent()
    hook.handleRowDragStart(source, event)
    expect(event.preventDefault).toHaveBeenCalled()
    const cloudEvent = createDragEvent()
    hook.handleRowDragStart({ path: 'rclone://remote/other', kind: 'file' } as never, cloudEvent)
    expect(cloudEvent.dataTransfer!.setData).not.toHaveBeenCalledWith('text/uri-list', expect.anything())
    expect(deps.showToast).toHaveBeenCalledWith('Drag local files and cloud files separately')
  })

  it('exports ordinary drags as file URIs and never cancels them for Alt', () => {
    const { hook } = setup()
    for (const altKey of [false, true]) {
      const event = createDragEvent({ altKey })
      hook.handleRowDragStart(source, event)
      expect(event.preventDefault).not.toHaveBeenCalled()
      expect(event.dataTransfer!.setData).toHaveBeenCalledWith('text/uri-list', 'file:///tmp/source.txt\r\n')
      expect(event.dataTransfer!.effectAllowed).toBe('copyMove')
    }
  })

  it('routes a returning native self-drop once and preserves its mode across blur', async () => {
    const { hook, deps } = setup()
    await hook.startNativeDrop()
    target('/tmp/dest')
    hook.handleRowDragStart(source, createDragEvent({ shiftKey: true }))
    expect(get(hook.dragGhostVisible)).toBe(true)
    window.dispatchEvent(new Event('blur'))
    expect(get(hook.dragGhostVisible)).toBe(false)
    expect(get(hook.dragState).dragging).toBe(true)
    onNativeHover(['/tmp/source.txt'], point)
    expect(get(hook.dragGhostVisible)).toBe(true)
    expect(get(hook.dragAction)).toBe('move')
    await onNativeDrop(['/tmp/source.txt'], point)
    document.dispatchEvent(new MouseEvent('drop', { bubbles: true, cancelable: true }))
    expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/tmp/source.txt'], mode: 'cut' })
  })

  it.each([
    [{ ctrlKey: true }, { shiftKey: true }, 'copy', 'copy'],
    [{ shiftKey: true }, { ctrlKey: true }, 'cut', 'move'],
    [{ ctrlKey: true, shiftKey: true }, {}, 'copy', 'copy'],
  ] as const)('keeps the explicit start action consistent with the native offer (%j)', async (startKeys, dropKeys, mode, effect) => {
    const { hook, deps } = setup()
    const event = createDragEvent(startKeys)
    hook.handleRowDragStart(source, event)
    expect(event.dataTransfer!.effectAllowed).toBe(effect)
    await hook.handleBookmarkDrop('/tmp/dest', createDragEvent(dropKeys))
    expect(deps.handlePasteOrMove).toHaveBeenCalledWith('/tmp/dest', { paths: ['/tmp/source.txt'], mode })
  })

  it.each([['copy', { ctrlKey: true }, 'copy'], ['move', { shiftKey: true }, 'cut']] as const)(
    'preserves a successful %s source when DOM dragend precedes the matching native drop', async (effect, keys, mode) => {
      const { hook, deps } = setup()
      await hook.startNativeDrop()
      target('/tmp/dest')
      hook.handleRowDragStart(source, createDragEvent(keys))
      onNativeHover(['/tmp/source.txt'], point)
      const end = createDragEvent()
      end.dataTransfer!.dropEffect = effect
      const dispatched = new Event('dragend', { bubbles: true })
      Object.defineProperty(dispatched, 'dataTransfer', { value: end.dataTransfer })
      document.dispatchEvent(dispatched)
      // A component can receive the same bubbling dragend after document capture.
      hook.handleRowDragEnd(end)
      expect(get(hook.dragState).dragging).toBe(false)
      expect(get(hook.dragGhostVisible)).toBe(false)
      await onNativeDrop(['/tmp/source.txt'], point)
      expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/tmp/source.txt'], mode })
    },
  )

  it.each(['cancel', 'escape', 'new-press', 'expired', 'different-point', 'different-path'])(
    'does not apply a completed move action to a later external drop after %s', async reason => {
      const { hook, deps } = setup()
      await hook.startNativeDrop()
      target('/tmp/dest')
      hook.handleRowDragStart(source, createDragEvent({ shiftKey: true }))
      onNativeHover(['/tmp/source.txt'], point)
      const end = createDragEvent()
      end.dataTransfer!.dropEffect = reason === 'cancel' ? 'none' : 'move'
      hook.handleRowDragEnd(end)
      if (reason === 'escape') document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
      if (reason === 'new-press') document.dispatchEvent(new MouseEvent('pointerdown', { buttons: 1 }))
      if (reason === 'expired') await vi.advanceTimersByTimeAsync(2001)
      const paths = reason === 'different-path' ? ['/other/file'] : ['/tmp/source.txt']
      await onNativeDrop(paths, reason === 'different-point' ? { x: point.x + 1, y: point.y } : point)
      expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths, mode: 'copy' })
    },
  )

  it('does not replace an active internal source with an unrelated native selection', async () => {
    const { hook, deps } = setup()
    target('/tmp/dest')
    hook.handleRowDragStart(source, createDragEvent())
    await onNativeDrop(['/tmp/unrelated.txt'], point)
    expect(deps.handlePasteOrMove).not.toHaveBeenCalled()
  })

  it.each(['mouseup', 'pointerup', 'mousedown', 'pointerdown'])(
    'clears a missed dragend on ordinary %s input', async type => {
      const { hook, deps } = setup()
      await hook.startNativeDrop()
      target('/tmp/dest')
      hook.handleRowDragStart(source, createDragEvent({ shiftKey: true }))
      window.dispatchEvent(new Event('blur'))
      document.dispatchEvent(new MouseEvent(type, { bubbles: true, buttons: type.endsWith('down') ? 1 : 0 }))
      expect(get(hook.dragState).dragging).toBe(false)
      await onNativeDrop(['/other/file'], point)
      expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/other/file'], mode: 'copy' })
    },
  )

  it.each([
    ['mousemove', { shiftKey: true }, 'cut'],
    ['pointermove', { shiftKey: true }, 'cut'],
    ['mousemove', { ctrlKey: true }, 'copy'],
    ['pointermove', { ctrlKey: true }, 'copy'],
  ] as const)('preserves internal drag after zero-button %s motion (%j)', async (type, keys, mode) => {
    const { hook, deps } = setup()
    await hook.startNativeDrop()
    target('/tmp/dest')
    hook.handleRowDragStart(source, createDragEvent(keys))
    document.dispatchEvent(new MouseEvent(type, { bubbles: true, buttons: 0 }))
    expect(get(hook.dragState).dragging).toBe(true)
    window.dispatchEvent(new Event('blur'))
    document.dispatchEvent(new MouseEvent(type, { bubbles: true, buttons: 0 }))
    expect(get(hook.dragState).dragging).toBe(true)
    expect(get(hook.dragGhostVisible)).toBe(false)
    onNativeHover(['/tmp/source.txt'], point)
    document.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: 80, clientY: 90 }))
    expect(get(hook.dragState).position).toEqual({ x: 80, y: 90 })
    expect(get(hook.dragGhostVisible)).toBe(true)
    await onNativeDrop(['/tmp/source.txt'], point)
    expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/tmp/source.txt'], mode })
  })

  it.each(['hover', 'drop'])('accepts an unrelated native %s after the source leaves without dragend', async event => {
    const { hook, deps } = setup()
    await hook.startNativeDrop()
    target('/tmp/dest')
    hook.handleRowDragStart(source, createDragEvent({ shiftKey: true }))
    onNativeLeave()
    if (event === 'hover') onNativeHover(['/other/file'], point)
    await onNativeDrop(['/other/file'], point)
    expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/other/file'], mode: 'copy' })
  })

  it('keeps an active drag on held-button movement and removes input listeners on stop', async () => {
    const { hook } = setup()
    await hook.startNativeDrop()
    hook.handleRowDragStart(source, createDragEvent())
    document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, buttons: 1 }))
    expect(get(hook.dragState).dragging).toBe(true)
    await hook.stopNativeDrop()
    hook.handleRowDragStart(source, createDragEvent())
    document.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, buttons: 0 }))
    expect(get(hook.dragState).dragging).toBe(true)
  })

  it.each(['native', 'document', 'blur'])('hides the preview on %s exit without losing the source', async exit => {
    let resolve!: (mode: 'copy' | 'cut') => void
    resolveDropClipboardModeMock.mockReturnValue(new Promise(r => { resolve = r }))
    const { hook } = setup()
    await hook.startNativeDrop()
    const el = target('/tmp/dest')
    hook.handleRowDragStart(source, createDragEvent())
    hook.handleBookmarkDragOver('/tmp/dest', createDragEvent())
    expect(get(hook.dragGhostVisible)).toBe(true)
    if (exit === 'native') onNativeLeave()
    else if (exit === 'blur') window.dispatchEvent(new Event('blur'))
    else document.dispatchEvent(new MouseEvent('dragleave', { bubbles: true, relatedTarget: null }))
    resolve('cut')
    await vi.advanceTimersByTimeAsync(0)
    expect(get(hook.dragGhostVisible)).toBe(false)
    expect(get(hook.dragState).paths).toEqual(['/tmp/source.txt'])
    expect(get(hook.dragState).target).toBeNull()
    expect(get(hook.dragAction)).toBeNull()
    expect(el.hasAttribute('data-drop-active')).toBe(false)
    document.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: point.x, clientY: point.y }))
    expect(get(hook.dragGhostVisible)).toBe(true)
  })

  it('keeps the label alive on child dragleave with no relatedTarget', async () => {
    const { hook, deps } = setup()
    await hook.startNativeDrop()
    const file = target('')
    hook.handleRowDragStart(source, createDragEvent({ shiftKey: true }))
    file.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: 20, clientY: 30 }))
    const visibility = vi.fn()
    const stop = hook.dragGhostVisible.subscribe(visibility)
    try {
      visibility.mockClear()
      file.dispatchEvent(new MouseEvent('dragleave', { bubbles: true, relatedTarget: null }))
      file.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: 80, clientY: 90 }))
      expect(visibility).not.toHaveBeenCalled()
      expect(get(hook.dragState).position).toEqual({ x: 80, y: 90 })
      expect(get(hook.dragGhostVisible)).toBe(true)
      target('/tmp/dest')
      await onNativeDrop(['/tmp/source.txt'], point)
      expect(deps.handlePasteOrMove).toHaveBeenCalledExactlyOnceWith('/tmp/dest', { paths: ['/tmp/source.txt'], mode: 'cut' })
    } finally { stop() }
  })

  it('does not publish listing inputs or resolve a destination while moving over rejected files', async () => {
    const { hook } = setup()
    await hook.startNativeDrop()
    const file = target('')
    hook.handleRowDragStart(source, createDragEvent())
    const notifications = [vi.fn(), vi.fn(), vi.fn()]
    const stops = [hook.dragTargetPath, hook.dragPathsLength, hook.dragging].map((store, i) => store.subscribe(notifications[i]))
    try {
      notifications.forEach(spy => spy.mockClear())
      for (let i = 0; i < 100; i++) {
        file.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true, clientX: i, clientY: 30 }))
        file.dispatchEvent(new MouseEvent('dragleave', { bubbles: true, relatedTarget: null }))
      }
      notifications.forEach(spy => expect(spy).not.toHaveBeenCalled())
      expect(resolveDropClipboardModeMock).not.toHaveBeenCalled()
      expect(get(hook.dragState).position).toEqual({ x: 99, y: 30 })
    } finally { stops.forEach(stop => stop()) }
  })

  it.each(['internal-document', 'internal-native', 'external-native'] as const)(
    'retains a valid background target without republishing listing inputs (%s)', async origin => {
      let blocked = false
      const { hook } = setup({ isBlocked: () => blocked })
      await hook.startNativeDrop()
      const background = target()
      const paths = origin === 'external-native' ? ['/other/source.txt'] : ['/tmp/source.txt']
      if (origin !== 'external-native') hook.handleRowDragStart(source, createDragEvent())
      const over = (x: number) => {
        if (origin === 'internal-document') background.dispatchEvent(new MouseEvent('dragover', {
          bubbles: true, cancelable: true, clientX: x, clientY: 30,
        }))
        else onNativeHover(paths, { x, y: 30 })
      }
      over(20)
      await vi.advanceTimersByTimeAsync(0)
      const notifications = [vi.fn(), vi.fn(), vi.fn(), vi.fn()]
      const stops = [hook.dragTargetPath, hook.dragPathsLength, hook.dragging, hook.dragAction]
        .map((store, i) => store.subscribe(notifications[i]))
      const setAttribute = vi.spyOn(background, 'setAttribute')
      const removeAttribute = vi.spyOn(background, 'removeAttribute')
      try {
        notifications.forEach(spy => spy.mockClear())
        for (let i = 0; i < 100; i++) over(i)
        await vi.advanceTimersByTimeAsync(0)
        notifications.forEach(spy => expect(spy).not.toHaveBeenCalled())
        expect(setAttribute).not.toHaveBeenCalled()
        expect(removeAttribute).not.toHaveBeenCalled()
        expect(get(hook.dragState).target).toBe('/tmp')
        expect(get(hook.dragState).position).toEqual({ x: 99, y: 30 })
        expect(resolveDropClipboardModeMock).toHaveBeenCalledTimes(origin === 'external-native' ? 0 : 1)
        // Retaining feedback does not cache permission to drop when a dialog opens.
        blocked = true
        over(100)
        expect(get(hook.dragState).target).toBeNull()
        expect(get(hook.dragAction)).toBeNull()
        expect(background.hasAttribute('data-drop-active')).toBe(false)
      } finally { stops.forEach(stop => stop()) }
    },
  )

  it.each(['internal', 'external'])('hides the %s preview throughout an accepted asynchronous transfer', async origin => {
    let finish!: (result: boolean) => void
    const handlePasteOrMove = vi.fn(() => new Promise<boolean>(resolve => { finish = resolve }))
    const { hook } = setup({ handlePasteOrMove })
    await hook.startNativeDrop()
    target('/tmp/dest')
    const paths = origin === 'internal' ? ['/tmp/source.txt'] : ['/other/file']
    if (origin === 'internal') hook.handleRowDragStart(source, createDragEvent({ shiftKey: true }))
    onNativeHover(paths, point)
    expect(get(hook.dragGhostVisible)).toBe(true)
    const drop = onNativeDrop(paths, point)
    expect(get(hook.dragGhostVisible)).toBe(false)
    await vi.advanceTimersByTimeAsync(0)
    onNativeLeave()
    onNativeHover(paths, point)
    document.dispatchEvent(new MouseEvent('dragover', { bubbles: true, cancelable: true }))
    document.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, buttons: 0 }))
    document.dispatchEvent(new Event('dragend', { bubbles: true }))
    await onNativeDrop(paths, point)
    expect(get(hook.dragGhostVisible)).toBe(false)
    expect(get(hook.dragState).dragging).toBe(true)
    expect(handlePasteOrMove).toHaveBeenCalledTimes(1)
    finish(true)
    await drop
    expect(get(hook.dragState).dragging).toBe(false)
  })
})

describe('cross-instance cloud copies', () => {
  const token = 'ab'.repeat(32)
  const offer = [`browsey-drag://cloud/${token}`]
  const source = 'rclone://Google Disk//gdrive/~folder/selected-id~same%20name.txt'
  beforeEach(() => { vi.clearAllMocks(); resolveCloudDragMock.mockResolvedValue([source]) })
  afterEach(() => { document.body.innerHTML = '' })
  const setup = (dest = 'rclone://Onedrive/destination') => {
    const background = document.createElement('div'); background.dataset.dropPath = dest; document.body.appendChild(background)
    document.elementFromPoint = vi.fn(() => background)
    const paste = vi.fn(async () => true), trash = vi.fn(async () => true), toast = vi.fn()
    const drag = useExplorerDragDrop({ currentView: () => 'dir', currentPath: () => dest,
      getSelectedSet: () => new Set(), loadDir: vi.fn(async () => {}), isBlocked: () => false,
      isSearchActive: () => false, handlePasteOrMove: paste, handleTrashDrop: trash, showToast: toast })
    return { drag, paste, trash, toast }
  }
  it('verifies at drop, preserves the Google Drive identity and copies once', async () => {
    const { drag, paste } = setup()
    onNativeHover(offer, { x: 12, y: 24 })
    await Promise.resolve(); await Promise.resolve()
    expect(get(drag.dragState).paths).toEqual([source])
    await onNativeDrop(offer, { x: 12, y: 24 })
    expect(resolveCloudDragMock).toHaveBeenLastCalledWith(token, true)
    expect(paste).toHaveBeenCalledExactlyOnceWith('rclone://Onedrive/destination', { paths: [source], mode: 'copy' })
    await onNativeDrop(offer, { x: 12, y: 24 })
    expect(paste).toHaveBeenCalledTimes(1)
  })
  it('keeps an internal cloud move and ignores its late native duplicate', async () => {
    vi.useFakeTimers()
    Object.defineProperty(window, '__BROWSEY_FILE_DRAG_BRIDGE__', { value: true, configurable: true })
    try {
      const { drag, paste } = setup()
      const release = vi.fn()
      prepareCloudDragMock.mockReturnValue({ token, payload: offer[0], ready: Promise.resolve(), release })
      drag.handleRowDragStart({ path: source, kind: 'file', name: 'same name.txt' } as never, createDragEvent({ shiftKey: true }))
      await drag.handleBookmarkDrop('rclone://Onedrive/destination', createDragEvent({ shiftKey: true }))
      expect(paste).toHaveBeenCalledExactlyOnceWith('rclone://Onedrive/destination', { paths: [source], mode: 'cut' })
      await onNativeDrop(offer, { x: 12, y: 24 })
      expect(paste).toHaveBeenCalledTimes(1)
      expect(resolveCloudDragMock).not.toHaveBeenCalled()
      expect(release).toHaveBeenCalledTimes(1)
      vi.runAllTimers()
    } finally {
      Reflect.deleteProperty(window, '__BROWSEY_FILE_DRAG_BRIDGE__')
      vi.clearAllTimers(); vi.useRealTimers()
    }
  })
  it('refuses Wastebasket without resolving or deleting a cross-instance offer', async () => {
    const { paste, trash } = setup('trash://')
    await onNativeDrop(offer, { x: 12, y: 24 })
    expect(resolveCloudDragMock).not.toHaveBeenCalled()
    expect(paste).not.toHaveBeenCalled(); expect(trash).not.toHaveBeenCalled()
  })
  it('account verification failures cannot start a transfer', async () => {
    const { paste, drag } = setup()
    resolveCloudDragMock.mockRejectedValue(new Error('account mismatch'))
    await expect(onNativeDrop(offer, { x: 12, y: 24 })).rejects.toThrow('account mismatch')
    expect(paste).not.toHaveBeenCalled(); expect(get(drag.dragState).dragging).toBe(false)
  })
  it('ignores late metadata after leave and rejects unverified raw cloud paths', async () => {
    const { drag, paste } = setup()
    let finish!: (paths: string[]) => void
    resolveCloudDragMock.mockReturnValue(new Promise<string[]>(resolve => { finish = resolve }))
    onNativeHover(offer, { x: 12, y: 24 }); onNativeLeave(); finish([source]); await Promise.resolve()
    expect(get(drag.dragState).dragging).toBe(false)
    await onNativeDrop([source], { x: 12, y: 24 })
    await onNativeDrop(['browsey-drag://cloud/bad'], { x: 12, y: 24 })
    expect(paste).not.toHaveBeenCalled()
  })
  it('does not mistake an unverified cloud URI for an active source returning', async () => {
    const { drag, paste } = setup()
    drag.handleRowDragStart({ path: source, kind: 'file', name: 'same name.txt' } as never, createDragEvent())
    onNativeLeave()
    await onNativeDrop([source], { x: 12, y: 24 })
    expect(paste).not.toHaveBeenCalled()
  })
})

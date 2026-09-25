import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get, writable } from 'svelte/store'

const setClipboardPathsStateMock = vi.fn()
const setClipboardCmdMock = vi.fn()
const resolveDropClipboardModeMock = vi.fn()
const startNativeFileDragMock = vi.fn()
let onNativeDrop: (paths: string[]) => Promise<void>

vi.mock('svelte', async () => {
  const actual = await vi.importActual<typeof import('svelte')>('svelte')
  return {
    ...actual,
    onDestroy: () => {},
  }
})

vi.mock('./createNativeFileDrop', () => ({
  createNativeFileDrop: vi.fn((options: { onDrop: typeof onNativeDrop }) => {
    onNativeDrop = options.onDrop
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
    dataTransfer: createDataTransfer(),
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    ...overrides,
  }) as unknown as DragEvent

describe('useExplorerDragDrop bookmark drop handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resolveDropClipboardModeMock.mockResolvedValue('copy')
    setClipboardCmdMock.mockResolvedValue(undefined)
    startNativeFileDragMock.mockResolvedValue(true)
  })

  it('drops onto bookmark paths through the same paste flow as breadcrumbs', async () => {
    const handlePasteOrMove = vi.fn(async () => true)
    const dragDrop = useExplorerDragDrop({
      currentView: () => 'dir',
      currentPath: () => '/tmp/current',
      getSelectedSet: () => new Set(['/tmp/source.txt']),
      loadDir: vi.fn(async () => {}),
      focusEntryInCurrentList: vi.fn(),
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
      focusEntryInCurrentList: vi.fn(), showToast: vi.fn(), handlePasteOrMove,
    })
    await onNativeDrop(['/tmp/dropped.jpg'])
    expect(handlePasteOrMove).toHaveBeenCalledWith(dest, { paths: ['/tmp/dropped.jpg'], mode: 'copy' })
    expect(setClipboardPathsStateMock).not.toHaveBeenCalled()
    expect(setClipboardCmdMock).not.toHaveBeenCalled()
  })
})

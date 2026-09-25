import { onDestroy } from 'svelte'
import { get, writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { useDragDrop } from './useDragDrop'
import { createNativeFileDrop, type DropPosition } from './createNativeFileDrop'
import { createDragNavigation } from './createDragNavigation'
import { findDropTarget, isDropDirectoryPath } from './dropTargets'
import { normalizePath } from '../utils'
import { resolveDropClipboardMode, type PasteSources } from '../services/clipboard.service'
import { fileDragStartMode } from './fileDragPayload'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'

type Deps = {
  currentView: () => CurrentView
  currentPath: () => string
  getSelectedSet: () => Set<string>
  loadDir: (path: string) => Promise<void>
  isBlocked: () => boolean
  isSearchActive: () => boolean
  handlePasteOrMove: (dest: string, input: PasteSources) => Promise<boolean>
  showToast: (msg: string, durationMs?: number) => void
}

type Mode = 'copy' | 'cut'
type DragAction = 'copy' | 'move' | null
type Modifiers = Pick<DragEvent, 'ctrlKey' | 'metaKey' | 'shiftKey'>
const isCloudPath = (path: string) => path.startsWith('rclone://')
const mixedSelection = (paths: string[]) => paths.some(isCloudPath) && !paths.every(isCloudPath)
const noModifiers: Modifiers = { ctrlKey: false, metaKey: false, shiftKey: false }

export const useExplorerDragDrop = (deps: Deps) => {
  const dragDrop = useDragDrop()
  const dragState = dragDrop.state
  const dragAction = writable<DragAction>(null)
  let dragPaths: string[] = []
  let external = false
  let navigating = false
  let transferring = false
  let session = 0
  let previewToken = 0
  let modifiers = noModifiers
  let sourceMode: Mode | null = null
  let lastPoint: DropPosition | null = null
  let hoverOpenedAt: DropPosition | null = null
  let highlighted: HTMLElement | null = null
  let listening = false
  const modeCache = new Map<string, Promise<Mode>>()
  const resolvedModes = new Map<string, Mode>()
  const blocked = () => deps.isBlocked() || navigating || transferring

  const clearTarget = () => {
    highlighted?.removeAttribute('data-drop-active')
    highlighted = null
    dragDrop.setTarget(null)
    dragAction.set(null)
    previewToken += 1
  }

  const navigation = createDragNavigation({
    canNavigate: () => !blocked() && dragPaths.length > 0,
    open: (path) => {
      if (normalizePath(path) === normalizePath(deps.currentPath())) return
      const token = session
      navigating = true
      navigation.stop()
      clearTarget()
      void deps.loadDir(path).catch(error => {
        deps.showToast(`Could not open folder: ${getErrorMessage(error)}`)
      }).finally(() => {
        navigating = false
        // Do not auto-open a second folder that appears under a stationary pointer.
        if (token === session) hoverOpenedAt = lastPoint ? { ...lastPoint } : null
      })
    },
    onScroll: (point) => updateAt(point),
  })

  const handleRowDragEnd = () => {
    session += 1
    dragPaths = []
    external = false
    modifiers = noModifiers
    sourceMode = null
    lastPoint = null
    hoverOpenedAt = null
    navigation.stop()
    clearTarget()
    dragDrop.end()
    modeCache.clear()
    resolvedModes.clear()
  }

  const canDrop = (paths: string[], dest: string) => !blocked() && isDropDirectoryPath(dest)
    && !mixedSelection(paths) && dragDrop.canDropOn(paths, dest)

  // External native events carry no modifier keys: always copy incoming files.
  // An explicit local start action matches the native offer. Otherwise use the
  // current internal drop event, never stale keyboard state.
  const resolveMode = (paths: string[], dest: string, keys: Modifiers, native = external): Promise<Mode> => {
    if (native) return Promise.resolve('copy')
    if (sourceMode) return Promise.resolve(sourceMode)
    if (keys.ctrlKey || keys.metaKey) return Promise.resolve('copy')
    if (keys.shiftKey) return Promise.resolve('cut')
    if (paths.some(isCloudPath) || isCloudPath(dest)) return Promise.resolve('copy')
    const key = JSON.stringify([paths, dest])
    const cached = modeCache.get(key)
    if (cached) return cached
    const token = session
    const result = resolveDropClipboardMode(paths, dest, false).then(mode => {
      if (token === session) resolvedModes.set(key, mode)
      return mode
    })
    modeCache.set(key, result)
    return result
  }

  const preview = (dest: string, event?: DragEvent) => {
    const token = ++previewToken
    dragDrop.setTarget(dest)
    const known = external ? 'copy' : sourceMode ?? (modifiers.ctrlKey || modifiers.metaKey ? 'copy'
      : modifiers.shiftKey ? 'cut'
      : dragPaths.some(isCloudPath) || isCloudPath(dest) ? 'copy'
      : resolvedModes.get(JSON.stringify([dragPaths, dest])))
    const action = known === 'copy' ? 'copy' : known === 'cut' ? 'move' : null
    dragAction.set(action)
    // The browser only reads dropEffect synchronously during dragover.
    if (event?.dataTransfer) event.dataTransfer.dropEffect = action ?? 'none'
    void resolveMode([...dragPaths], dest, modifiers).then(mode => {
      if (token !== previewToken || get(dragState).target !== dest || blocked()) return
      const action = mode === 'copy' ? 'copy' : 'move'
      dragAction.set(action)
    }).catch(() => {
      if (token === previewToken) clearTarget()
    })
  }

  const targetAt = (point: DropPosition) => findDropTarget(point,
    deps.currentView() === 'dir' && !deps.isSearchActive() ? deps.currentPath() : null)

  const updateAt = (point: DropPosition, event?: DragEvent) => {
    if (hoverOpenedAt && (hoverOpenedAt.x !== point.x || hoverOpenedAt.y !== point.y)) hoverOpenedAt = null
    lastPoint = point
    dragDrop.setPosition(point.x, point.y)
    const target = blocked() ? null : targetAt(point)
    const allowed = target && canDrop(dragPaths, target.path) ? target : null
    clearTarget()
    if (allowed) {
      highlighted = allowed.element
      highlighted.setAttribute('data-drop-active', 'true')
      preview(allowed.path, event)
    } else if (event?.dataTransfer) event.dataTransfer.dropEffect = 'none'
    if (blocked()) navigation.stop()
    else navigation.update(point, !hoverOpenedAt && allowed && allowed.path !== deps.currentPath() ? allowed : null)
  }

  const performDrop = async (dest: string | null, paths: string[], keys: Modifiers, native: boolean) => {
    if (transferring) return
    const accepted = dest !== null && canDrop(paths, dest)
    const token = session
    navigation.stop()
    clearTarget()
    if (!accepted || !dest) {
      handleRowDragEnd()
      return
    }
    transferring = true
    try {
      const mode = await resolveMode(paths, dest, keys, native)
      if (token !== session || deps.isBlocked()) return
      // Snapshot destination, sources and action before entering conflict/paste orchestration.
      await deps.handlePasteOrMove(dest, { paths, mode })
    } catch (error) {
      deps.showToast(`Drop failed: ${getErrorMessage(error)}`)
    } finally {
      transferring = false
      handleRowDragEnd()
    }
  }

  const nativeDrop = createNativeFileDrop({
    onHover: (paths, point) => {
      if (dragPaths.length && !external) {
        if (paths.length === dragPaths.length && paths.every((path, i) => path === dragPaths[i])) updateAt(point)
        return
      }
      external = true
      dragPaths = [...paths]
      dragState.set({ dragging: paths.length > 0, paths: [...paths], target: null, position: point })
      updateAt(point)
    },
    onLeave: () => {
      if (external && !transferring) handleRowDragEnd()
      else { navigation.stop(); clearTarget() }
    },
    onDrop: async (paths, point) => {
      const dest = blocked() ? null : targetAt(point)?.path ?? null
      if (dragPaths.length && !external) {
        // URI exports can re-enter this same webview through Tauri's native drop
        // interceptor. Preserve internal modifiers and route exactly once.
        if (paths.length === dragPaths.length && paths.every((path, i) => path === dragPaths[i])) {
          await performDrop(dest, [...dragPaths], modifiers, false)
        }
        return
      }
      await performDrop(dest, [...paths], noModifiers, true)
    },
    onError: error => deps.showToast(`Drop failed: ${getErrorMessage(error)}`),
  })

  const handleRowDragStart = (entry: Entry, event: DragEvent) => {
    if (blocked() || deps.currentView() === 'network' || deps.currentView() === 'trash') {
      event.preventDefault()
      return
    }
    const selected = deps.getSelectedSet()
    const paths = selected.has(entry.path) ? [...selected] : [entry.path]
    if (mixedSelection(paths)) {
      event.preventDefault()
      deps.showToast('Drag local files and cloud files separately')
      return
    }
    handleRowDragEnd()
    dragPaths = paths
    sourceMode = paths.some(isCloudPath) ? null : fileDragStartMode(event)
    modifiers = event
    dragDrop.start(paths, event)
  }

  // Document capture gives every destination (including empty list/grid space and
  // mounted sidebar drives) identical guards and prevents bubbling double transfers.
  const handleDocumentDragOver = (event: DragEvent) => {
    if (!dragPaths.length || external) return
    event.preventDefault()
    event.stopPropagation()
    modifiers = event
    updateAt({ x: event.clientX, y: event.clientY }, event)
  }
  const handleDocumentDrop = (event: DragEvent) => {
    if (!dragPaths.length || external) return
    event.preventDefault()
    event.stopPropagation()
    const target = blocked() ? null : targetAt({ x: event.clientX, y: event.clientY })
    void performDrop(target?.path ?? null, [...dragPaths], event, false)
  }
  const handleDocumentLeave = (event: DragEvent) => {
    if (event.relatedTarget !== null) return
    navigation.stop()
    clearTarget()
    lastPoint = null
  }
  const handleBlur = () => {
    if (dragPaths.length && !external && !transferring) {
      // Crossing into another app must not discard the source session. Native
      // dragend/Escape completes it; re-entering Browsey still has its snapshot.
      navigation.stop()
      clearTarget()
      lastPoint = null
      return
    }
    if (!transferring) {
      handleRowDragEnd()
      return
    }
    modifiers = noModifiers
    navigation.stop()
    clearTarget()
    lastPoint = null
  }
  const handleKey = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      handleRowDragEnd()
      return
    }
    if (!dragPaths.length || external) return
    modifiers = event
    if (lastPoint) updateAt(lastPoint)
  }
  const endUnlessTransferring = () => { if (!transferring) handleRowDragEnd() }
  const startNativeDrop = async () => {
    if (!listening) {
      listening = true
      document.addEventListener('dragover', handleDocumentDragOver, true)
      document.addEventListener('drop', handleDocumentDrop, true)
      document.addEventListener('dragleave', handleDocumentLeave, true)
      document.addEventListener('dragend', endUnlessTransferring, true)
      document.addEventListener('keydown', handleKey, true)
      document.addEventListener('keyup', handleKey, true)
      window.addEventListener('blur', handleBlur)
    }
    await nativeDrop.start()
  }
  const stopNativeDrop = async () => {
    listening = false
    document.removeEventListener('dragover', handleDocumentDragOver, true)
    document.removeEventListener('drop', handleDocumentDrop, true)
    document.removeEventListener('dragleave', handleDocumentLeave, true)
    document.removeEventListener('dragend', endUnlessTransferring, true)
    document.removeEventListener('keydown', handleKey, true)
    document.removeEventListener('keyup', handleKey, true)
    window.removeEventListener('blur', handleBlur)
    handleRowDragEnd()
    await nativeDrop.stop()
  }
  onDestroy(() => { void stopNativeDrop() })

  // Component adapters also work independently of document listeners (e.g. tests).
  const handlePathDragOver = (path: string, event: DragEvent) => {
    modifiers = event
    event.preventDefault()
    if (canDrop(dragPaths, path)) preview(path, event)
    else {
      clearTarget()
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'none'
    }
    dragDrop.setPosition(event.clientX, event.clientY)
  }
  const handlePathDrop = async (path: string, event: DragEvent) => {
    event.preventDefault()
    event.stopPropagation()
    await performDrop(path, [...dragPaths], event, false)
  }
  const handlePathLeave = (path: string, event?: DragEvent) => {
    if (event?.currentTarget instanceof Node && event.relatedTarget instanceof Node
      && event.currentTarget.contains(event.relatedTarget)) return
    if (get(dragState).target === path) clearTarget()
  }
  const handleRowDragOver = (entry: Entry, event: DragEvent) => {
    if (entry.kind === 'dir') handlePathDragOver(entry.path, event)
    else clearTarget()
  }
  return {
    dragState, dragAction, startNativeDrop, stopNativeDrop,
    handleRowDragStart, handleRowDragEnd: endUnlessTransferring,
    handleRowDragOver, handleRowDragEnter: handleRowDragOver,
    handleRowDrop: (entry: Entry, event: DragEvent) => entry.kind === 'dir'
      ? handlePathDrop(entry.path, event) : Promise.resolve(),
    handleRowDragLeave: (entry: Entry, event: DragEvent) => handlePathLeave(entry.path, event),
    handleBreadcrumbDragOver: handlePathDragOver, handleBookmarkDragOver: handlePathDragOver,
    handleBreadcrumbDrop: handlePathDrop, handleBookmarkDrop: handlePathDrop,
    handleBreadcrumbDragLeave: handlePathLeave, handleBookmarkDragLeave: handlePathLeave,
  }
}

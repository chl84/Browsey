import { onDestroy } from 'svelte'
import { get, writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { useDragDrop } from './useDragDrop'
import { createNativeFileDrop } from './useNativeFileDrop'
import { normalizePath, parentPath } from '../utils'
import { resolveDropClipboardMode, setClipboardCmd } from '../services/clipboard.service'
import { startNativeFileDrag } from '../services/nativeDrag.service'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/useContextActions'

type Deps = {
  currentView: () => CurrentView
  currentPath: () => string
  getSelectedSet: () => Set<string>
  loadDir: (path: string) => Promise<void>
  focusEntryInCurrentList: (path: string) => void
  handlePasteOrMove: (dest: string) => Promise<boolean>
  showToast: (msg: string, durationMs?: number) => void
}

type DragAction = 'copy' | 'move' | null

export const useExplorerDragDrop = (deps: Deps) => {
  const dragDrop = useDragDrop()
  const dragState = dragDrop.state
  const dragAction = writable<DragAction>(null)

  let dragPaths: string[] = []
  let copyModifierActive = false
  const dropModePreviewCache = new Map<string, 'copy' | 'cut'>()
  const dropModePreviewInflight = new Map<string, Promise<'copy' | 'cut'>>()
  let dropModePreviewToken = 0

  const nativeDrop = createNativeFileDrop({
    onDrop: async (paths) => {
      if (!paths || paths.length === 0) return
      const curr = deps.currentPath()
      const view = deps.currentView()
      if (view === 'dir' && curr) {
        try {
          await setClipboardCmd(paths, 'copy')
          const ok = await deps.handlePasteOrMove(curr)
          if (ok) {
            deps.showToast(`Pasted ${paths.length} item${paths.length === 1 ? '' : 's'}`)
          }
        } catch (err) {
          deps.showToast(`Drop failed: ${getErrorMessage(err)}`)
        }
        return
      }
      const first = paths[0]
      const dest = parentPath(first)
      await deps.loadDir(dest)
      deps.focusEntryInCurrentList(first)
      deps.showToast('Dropped item navigated')
    },
  })

  const unsubscribeHover = nativeDrop.hovering.subscribe((hovering) => {
    if (hovering && deps.currentView() === 'dir') {
      deps.showToast('Drop to paste into this folder', 1500)
    }
  })
  onDestroy(() => {
    unsubscribeHover()
  })

  const setCopyModifierActive = (active: boolean) => {
    copyModifierActive = active
  }

  const clearDropModePreview = () => {
    dropModePreviewCache.clear()
    dropModePreviewInflight.clear()
    dropModePreviewToken += 1
  }

  const handleRowDragEnd = () => {
    dragPaths = []
    dragDrop.end()
    dragAction.set(null)
    clearDropModePreview()
  }

  const dropModifierPrefersCopy = (_event: DragEvent) => copyModifierActive

  const dropModeCacheKey = (paths: string[], dest: string, preferCopy: boolean) =>
    `${preferCopy ? '1' : '0'}|${normalizePath(dest)}|${paths
      .map((path) => normalizePath(path))
      .sort()
      .join('\u0000')}`

  const modeToDragAction = (mode: 'copy' | 'cut'): Exclude<DragAction, null> =>
    mode === 'copy' ? 'copy' : 'move'

  const resolveDropModeCached = async (
    paths: string[],
    dest: string,
    preferCopy: boolean,
  ): Promise<'copy' | 'cut'> => {
    const key = dropModeCacheKey(paths, dest, preferCopy)
    const cached = dropModePreviewCache.get(key)
    if (cached) return cached
    const inflight = dropModePreviewInflight.get(key)
    if (inflight) return inflight
    const request = resolveDropClipboardMode(paths, dest, preferCopy)
      .then((mode) => {
        dropModePreviewCache.set(key, mode)
        return mode
      })
      .finally(() => {
        dropModePreviewInflight.delete(key)
      })
    dropModePreviewInflight.set(key, request)
    return request
  }

  const previewDropAction = (paths: string[], dest: string, event: DragEvent) => {
    if (paths.length === 0) {
      dragAction.set(null)
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'none'
      return
    }
    const preferCopy = dropModifierPrefersCopy(event)
    const key = dropModeCacheKey(paths, dest, preferCopy)
    const cached = dropModePreviewCache.get(key)
    if (cached) {
      const action = modeToDragAction(cached)
      dragAction.set(action)
      if (event.dataTransfer) event.dataTransfer.dropEffect = action
      return
    }

    const fallback = preferCopy ? 'copy' : 'move'
    dragAction.set(fallback)
    if (event.dataTransfer) event.dataTransfer.dropEffect = fallback

    const token = ++dropModePreviewToken
    void resolveDropModeCached(paths, dest, preferCopy)
      .then((mode) => {
        if (token !== dropModePreviewToken) return
        if (get(dragState).target !== dest) return
        dragAction.set(modeToDragAction(mode))
      })
      .catch(() => {
        // Keep fallback action when mode preview resolution fails.
      })
  }

  const resolveDropMode = async (
    paths: string[],
    dest: string,
    event: DragEvent,
  ): Promise<'copy' | 'cut'> => {
    const preferCopy = dropModifierPrefersCopy(event)
    return resolveDropModeCached(paths, dest, preferCopy)
  }

  const handleRowDragStart = (entry: Entry, event: DragEvent) => {
    if (deps.currentView() === 'network') {
      event.preventDefault()
      return
    }
    const selectedSet = deps.getSelectedSet()
    const selectedPaths = selectedSet.has(entry.path) ? Array.from(selectedSet) : [entry.path]
    const nativeCopy = event.ctrlKey || event.metaKey
    if (event.altKey) {
      event.preventDefault()
      event.stopPropagation()
      void startNativeFileDrag(selectedPaths, nativeCopy ? 'copy' : 'move').then((ok) => {
        if (!ok) deps.showToast('Native drag failed')
      })
      event.preventDefault()
      return
    }
    dragPaths = selectedPaths
    dragDrop.start(selectedPaths, event)
    if (!event.ctrlKey && !event.metaKey) {
      copyModifierActive = false
    }
    dragAction.set(null)
  }

  const handleRowDragOver = (entry: Entry, event: DragEvent) => {
    if (entry.kind !== 'dir') return
    const allowed = dragDrop.canDropOn(dragPaths, entry.path)
    dragDrop.setTarget(allowed ? entry.path : null)
    if (allowed) {
      previewDropAction([...dragPaths], entry.path, event)
    } else {
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'none'
      dragAction.set(null)
    }
    dragDrop.setPosition(event.clientX, event.clientY)
  }

  const handleRowDragEnter = (entry: Entry, event: DragEvent) => {
    handleRowDragOver(entry, event)
  }

  const handleRowDragLeave = (entry: Entry, event: DragEvent) => {
    const target = event.currentTarget as HTMLElement | null
    const related = event.relatedTarget as HTMLElement | null
    if (target && related && target.contains(related)) {
      return
    }
    if (dragDrop.canDropOn(dragPaths, entry.path)) {
      dragDrop.setTarget(null)
      dragAction.set(null)
      dropModePreviewToken += 1
    }
  }

  const handleRowDrop = async (entry: Entry, event: DragEvent) => {
    if (entry.kind !== 'dir') return
    if (!dragDrop.canDropOn(dragPaths, entry.path)) return
    event.preventDefault()
    const sourcePaths = [...dragPaths]
    if (sourcePaths.length === 0) return
    try {
      const mode = await resolveDropMode(sourcePaths, entry.path, event)
      await setClipboardCmd(sourcePaths, mode)
      await deps.handlePasteOrMove(entry.path)
    } catch (err) {
      deps.showToast(`Drop failed: ${getErrorMessage(err)}`)
    } finally {
      handleRowDragEnd()
    }
  }

  const handleBreadcrumbDragOver = (path: string, event: DragEvent) => {
    if (dragPaths.length === 0) return
    const allowed = dragDrop.canDropOn(dragPaths, path)
    dragDrop.setTarget(allowed ? path : null)
    if (allowed) {
      previewDropAction([...dragPaths], path, event)
    } else {
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'none'
      dragAction.set(null)
    }
    dragDrop.setPosition(event.clientX, event.clientY)
    event.preventDefault()
  }

  const handleBreadcrumbDragLeave = (path: string) => {
    if (get(dragState).target === path) {
      dragDrop.setTarget(null)
    }
    dragAction.set(null)
    dropModePreviewToken += 1
  }

  const handleBreadcrumbDrop = async (path: string, event: DragEvent) => {
    if (dragPaths.length === 0) return
    if (!dragDrop.canDropOn(dragPaths, path)) return
    event.preventDefault()
    const sourcePaths = [...dragPaths]
    if (sourcePaths.length === 0) return
    try {
      const mode = await resolveDropMode(sourcePaths, path, event)
      await setClipboardCmd(sourcePaths, mode)
      await deps.handlePasteOrMove(path)
    } catch (err) {
      deps.showToast(`Drop failed: ${getErrorMessage(err)}`)
    } finally {
      handleRowDragEnd()
    }
  }

  return {
    dragState,
    dragAction,
    startNativeDrop: () => nativeDrop.start(),
    stopNativeDrop: () => nativeDrop.stop(),
    setCopyModifierActive,
    handleRowDragStart,
    handleRowDragEnd,
    handleRowDragEnter,
    handleRowDragOver,
    handleRowDrop,
    handleRowDragLeave,
    handleBreadcrumbDragOver,
    handleBreadcrumbDragLeave,
    handleBreadcrumbDrop,
  }
}

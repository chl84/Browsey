import { hitTestGridVirtualized } from '../selection/lassoHitTest'
import { isScrollbarClick } from '../helpers/scrollbar'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'
import { createGridKeyboardHandler } from './createGridKeyboardHandler'

type ViewMode = 'list' | 'grid'
type Mode = 'address' | 'filter'

type Deps = {
  getViewMode: () => ViewMode
  getMode: () => Mode
  setPathInput: (value: string) => void
  getPathInput: () => string
  isInputFocused: () => boolean
  getCurrentPath: () => string
  isSearchSessionEnabled: () => boolean
  getRowsEl: () => HTMLDivElement | null
  getHeaderEl: () => HTMLDivElement | null
  getFilteredEntries: () => Entry[]
  getSelected: () => Set<string>
  setSelected: (next: Set<string>) => void
  getAnchorIndex: () => number | null
  setAnchorIndex: (next: number | null) => void
  getCaretIndex: () => number | null
  setCaretIndex: (next: number | null) => void
  getRowHeight: () => number
  getDoubleClickMs: () => number
  setCopyModifierActive: (active: boolean) => void
  isEditableTarget: (target: EventTarget | null) => boolean
  hasAppShortcut: (event: KeyboardEvent) => boolean
  handleGlobalKeydown: (event: KeyboardEvent) => void | Promise<void>
  transitionToAddressMode: (options?: {
    path?: string
    blur?: boolean
    reloadOnDisable?: boolean
    skipToggle?: boolean
  }) => Promise<void>
  blurPathInput: () => void
  ensureGridVisible: (index: number) => void
  getRowsKeydownHandler: () => ((event: KeyboardEvent) => void) | null
  getRowSelectionHandler: () => ((entry: Entry, absoluteIndex: number, event: MouseEvent) => void) | null
  selectionBox: ReturnType<typeof import('../selection/createSelectionBox').createSelectionBox>
  getGridCols: () => number
  getGridCardWidth: () => number
  getGridRowHeight: () => number
  getGridGap: () => number
  handleRowsScroll: () => void
  handleGridScroll: () => void
  handleWheel: (event: WheelEvent) => void
  handleGridWheel: (event: WheelEvent) => void
  handleRowsClick: (event: MouseEvent) => void
  currentView: () => CurrentView
  loadDir: (path: string) => Promise<void>
  openPartition: (path: string) => Promise<void>
  canExtractPaths: (paths: string[]) => Promise<boolean>
  extractEntries: (entries: Entry[]) => Promise<void>
  open: (entry: Entry) => void | Promise<void>
  isBlockingModalOpen: () => boolean
  isDeleteModalOpen: () => boolean
  closeDeleteModal: () => void
  isRenameModalOpen: () => boolean
  closeRenameModal: () => void
  isOpenWithModalOpen: () => boolean
  closeOpenWithModal: () => void
  isPropertiesModalOpen: () => boolean
  closePropertiesModal: () => void
  isCompressModalOpen: () => boolean
  closeCompressModal: () => void
  isCheckDuplicatesModalOpen: () => boolean
  closeCheckDuplicatesModal: () => void
  isNewFolderModalOpen: () => boolean
  closeNewFolderModal: () => void
  isNewFileModalOpen: () => boolean
  closeNewFileModal: () => void
  isBookmarkModalOpen: () => boolean
  closeBookmarkModal: () => void
  isContextMenuOpen: () => boolean
  closeContextMenu: () => void
  isBlankMenuOpen: () => boolean
  closeBlankContextMenu: () => void
  suppressHoverMs?: number
}

export const useExplorerInputHandlers = (deps: Deps) => {
  let selectionDrag = false
  let pendingOpenCandidate: { path: string; atMs: number } | null = null
  let scrollHoverTimer: ReturnType<typeof setTimeout> | null = null

  const LASSO_GUTTER_WIDTH = 3
  const suppressHoverMs = deps.suppressHoverMs ?? 150

  const clearSelection = () => {
    deps.setSelected(new Set())
    deps.setAnchorIndex(null)
    deps.setCaretIndex(null)
  }

  const clearPendingOpenCandidate = () => {
    pendingOpenCandidate = null
  }

  const cleanupScrollHover = () => {
    if (scrollHoverTimer !== null) {
      clearTimeout(scrollHoverTimer)
      scrollHoverTimer = null
    }
    deps.getRowsEl()?.classList.remove('is-scrolling')
  }

  const inLassoGutter = (event: MouseEvent, el: HTMLElement | null) => {
    if (!el) return false
    const rect = el.getBoundingClientRect()
    return event.clientX <= rect.left + LASSO_GUTTER_WIDTH
  }

  const handleOpenEntry = async (entry: Entry) => {
    pendingOpenCandidate = null
    if (entry.kind === 'dir') {
      if (deps.currentView() === 'network') {
        await deps.transitionToAddressMode({ path: entry.path, reloadOnDisable: false })
        await deps.openPartition(entry.path)
      } else {
        await deps.transitionToAddressMode({ path: entry.path, reloadOnDisable: false })
        await deps.loadDir(entry.path)
      }
      return
    }
    if (entry.kind === 'file' && (await deps.canExtractPaths([entry.path]))) {
      await deps.extractEntries([entry])
      return
    }
    deps.open(entry)
  }

  const handleGridKeydown = createGridKeyboardHandler({
    getFilteredEntries: () => deps.getFilteredEntries(),
    selected: {
      subscribe: (run) => {
        run(deps.getSelected())
        return () => {}
      },
      set: (next: Set<string>) => deps.setSelected(next),
      update: (updater) => deps.setSelected(updater(deps.getSelected())),
    },
    anchorIndex: {
      subscribe: (run) => {
        run(deps.getAnchorIndex())
        return () => {}
      },
      set: (next: number | null) => deps.setAnchorIndex(next),
      update: (updater) => deps.setAnchorIndex(updater(deps.getAnchorIndex())),
    },
    caretIndex: {
      subscribe: (run) => {
        run(deps.getCaretIndex())
        return () => {}
      },
      set: (next: number | null) => deps.setCaretIndex(next),
      update: (updater) => deps.setCaretIndex(updater(deps.getCaretIndex())),
    },
    getGridCols: deps.getGridCols,
    ensureGridVisible: deps.ensureGridVisible,
    handleOpenEntry,
  })

  const isOpenClickCandidate = (event: MouseEvent) =>
    event.button === 0 &&
    !event.shiftKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey

  const clickTimestampMs = (event: MouseEvent) => {
    const stamp = Number(event.timeStamp)
    return Number.isFinite(stamp) ? stamp : performance.now()
  }

  const resolveDoubleClickMs = () => {
    const configured = deps.getDoubleClickMs()
    return Math.min(600, Math.max(150, Math.round(configured)))
  }

  const handleRowClickWithOpen = (entry: Entry, absoluteIndex: number, event: MouseEvent) => {
    deps.getRowSelectionHandler()?.(entry, absoluteIndex, event)

    if (!isOpenClickCandidate(event)) {
      pendingOpenCandidate = null
      return
    }

    const nowMs = clickTimestampMs(event)
    const thresholdMs = resolveDoubleClickMs()

    if (
      pendingOpenCandidate &&
      pendingOpenCandidate.path === entry.path &&
      nowMs - pendingOpenCandidate.atMs <= thresholdMs
    ) {
      pendingOpenCandidate = null
      void handleOpenEntry(entry)
      return
    }

    pendingOpenCandidate = { path: entry.path, atMs: nowMs }
  }

  const handleRowsMouseDown = (event: MouseEvent) => {
    const target = event.target as HTMLElement | null
    if (deps.getViewMode() === 'list') {
      const rowsEl = deps.getRowsEl()
      if (!rowsEl) return
      if (isScrollbarClick(event, rowsEl)) return
      if (inLassoGutter(event, rowsEl)) return
      if (target && target.closest('.row')) return
      event.preventDefault()
      rowsEl.focus()
      const list = deps.getFilteredEntries()
      if (list.length === 0) return
      selectionDrag = false
      const additive = event.ctrlKey || event.metaKey
      const subtractive = !additive && event.shiftKey
      const baseSelection = deps.getSelected()
      const baseAnchor = deps.getAnchorIndex()
      const baseCaret = deps.getCaretIndex()
      deps.selectionBox.start(event, {
        rowsEl,
        headerEl: deps.getHeaderEl(),
        entries: list,
        rowHeight: deps.getRowHeight(),
        onSelect: (paths: Set<string>, anchor: number | null, caret: number | null) => {
          if (subtractive) {
            const next = new Set(baseSelection)
            for (const path of paths) next.delete(path)
            const anchorPath = baseAnchor !== null ? list[baseAnchor]?.path : null
            const caretPath = baseCaret !== null ? list[baseCaret]?.path : null
            deps.setAnchorIndex(anchorPath && next.has(anchorPath) ? baseAnchor : null)
            deps.setCaretIndex(caretPath && next.has(caretPath) ? baseCaret : null)
            deps.setSelected(next)
          } else if (additive) {
            const merged = new Set(baseSelection)
            for (const path of paths) merged.add(path)
            deps.setSelected(merged)
            deps.setAnchorIndex(baseAnchor ?? anchor)
            deps.setCaretIndex(baseCaret ?? caret)
          } else {
            deps.setSelected(paths)
            deps.setAnchorIndex(anchor)
            deps.setCaretIndex(caret)
          }
        },
        onEnd: (didDrag: boolean) => {
          selectionDrag = didDrag
        },
      })
      return
    }

    const gridEl = event.currentTarget as HTMLDivElement | null
    if (!gridEl) return
    if (isScrollbarClick(event, gridEl)) return
    if (inLassoGutter(event, gridEl)) return
    if (target && target.closest('.card')) return
    const gridEntries = deps.getFilteredEntries()
    if (gridEntries.length === 0) return
    const style = getComputedStyle(gridEl)
    const gridPaddingLeft = parseFloat(style.paddingLeft) || 0
    const gridPaddingTop = parseFloat(style.paddingTop) || 0
    event.preventDefault()
    deps.blurPathInput()
    gridEl.focus()
    selectionDrag = false
    const additive = event.ctrlKey || event.metaKey
    const subtractive = !additive && event.shiftKey
    const baseSelection = deps.getSelected()
    const baseAnchor = deps.getAnchorIndex()
    const baseCaret = deps.getCaretIndex()
    deps.selectionBox.start(event, {
      rowsEl: gridEl,
      headerEl: null,
      entries: gridEntries,
      rowHeight: 1,
      hitTest: (rect) =>
        hitTestGridVirtualized(rect, gridEntries, {
          gridCols: deps.getGridCols(),
          cardWidth: deps.getGridCardWidth(),
          cardHeight: deps.getGridRowHeight(),
          gap: deps.getGridGap(),
          paddingLeft: gridPaddingLeft,
          paddingTop: gridPaddingTop,
        }),
      onSelect: (paths: Set<string>, anchor: number | null, caret: number | null) => {
        if (subtractive) {
          const next = new Set(baseSelection)
          for (const path of paths) next.delete(path)
          const anchorPath = baseAnchor !== null ? gridEntries[baseAnchor]?.path : null
          const caretPath = baseCaret !== null ? gridEntries[baseCaret]?.path : null
          deps.setAnchorIndex(anchorPath && next.has(anchorPath) ? baseAnchor : null)
          deps.setCaretIndex(caretPath && next.has(caretPath) ? baseCaret : null)
          deps.setSelected(next)
        } else if (additive) {
          const merged = new Set(baseSelection)
          for (const path of paths) merged.add(path)
          deps.setSelected(merged)
          deps.setAnchorIndex(baseAnchor ?? anchor ?? null)
          deps.setCaretIndex(baseCaret ?? caret ?? null)
        } else {
          deps.setSelected(paths)
          deps.setAnchorIndex(anchor ?? null)
          deps.setCaretIndex(caret ?? null)
        }
      },
      onEnd: (didDrag: boolean) => {
        selectionDrag = didDrag
      },
    })
  }

  const suppressHoverWhileScrolling = () => {
    const el = deps.getRowsEl()
    if (!el) return
    el.classList.add('is-scrolling')
    if (scrollHoverTimer !== null) {
      clearTimeout(scrollHoverTimer)
    }
    scrollHoverTimer = setTimeout(() => {
      scrollHoverTimer = null
      deps.getRowsEl()?.classList.remove('is-scrolling')
    }, suppressHoverMs)
  }

  const handleRowsScrollCombined = () => {
    suppressHoverWhileScrolling()
    if (deps.getViewMode() === 'list') {
      deps.handleRowsScroll()
    } else {
      deps.handleGridScroll()
    }
  }

  const handleRowsKeydownCombined = (event: KeyboardEvent) => {
    if (deps.getViewMode() === 'list') {
      deps.getRowsKeydownHandler()?.(event)
    } else {
      handleGridKeydown(event)
    }
  }

  const handleWheelCombined = (event: WheelEvent) => {
    if (deps.getViewMode() === 'list') {
      deps.handleWheel(event)
    } else {
      deps.handleGridWheel(event)
    }
  }

  const handleRowsClickSafe = (event: MouseEvent) => {
    if (selectionDrag) {
      selectionDrag = false
      pendingOpenCandidate = null
      return
    }
    if (isScrollbarClick(event, deps.getRowsEl())) return
    if (deps.getViewMode() === 'grid') {
      const target = event.target as HTMLElement | null
      if (target && target.closest('.card')) return
      pendingOpenCandidate = null
      if (deps.getSelected().size > 0) {
        clearSelection()
      }
      return
    }
    pendingOpenCandidate = null
    deps.handleRowsClick(event)
  }

  const handleDocumentKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Control' || event.key === 'Meta') {
      deps.setCopyModifierActive(true)
    }
    if (event.defaultPrevented) {
      return
    }

    const key = event.key.toLowerCase()
    const rowsEl = deps.getRowsEl()
    const inRows = rowsEl?.contains(event.target as Node) ?? false
    const blockingModalOpen = deps.isBlockingModalOpen()

    if (blockingModalOpen && key !== 'escape') {
      return
    }

    if ((event.ctrlKey || event.metaKey) && !deps.isEditableTarget(event.target)) {
      if (key === 'control' || key === 'meta' || key === 'alt' || key === 'shift') {
        return
      }
      if (event.shiftKey && !event.altKey && key === 'i') {
        return
      }
      const hasAppShortcut = deps.hasAppShortcut(event)
      if (!hasAppShortcut) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
      event.preventDefault()
      event.stopPropagation()
    }

    if (
      key === 'tab' &&
      deps.getMode() === 'filter' &&
      !event.shiftKey &&
      rowsEl &&
      !inRows &&
      deps.getPathInput().length > 0
    ) {
      const list = deps.getFilteredEntries()
      if (list.length > 0) {
        event.preventDefault()
        event.stopPropagation()
        deps.setSelected(new Set([list[0].path]))
        deps.setAnchorIndex(0)
        deps.setCaretIndex(0)
        const selector =
          deps.getViewMode() === 'grid'
            ? '.card[data-index="0"]'
            : '.row-viewport .row[data-index="0"]'
        const targetEl = rowsEl.querySelector<HTMLElement>(selector)
        if (targetEl) {
          targetEl.focus()
          targetEl.scrollIntoView({ block: 'nearest' })
        } else {
          rowsEl.focus()
        }
        if (deps.getViewMode() === 'grid') {
          deps.ensureGridVisible(0)
        }
        return
      }
    }

    if (key === 'enter' && !deps.isEditableTarget(event.target) && !inRows) {
      const list = deps.getFilteredEntries()
      if (list.length > 0 && deps.getSelected().size > 0) {
        event.preventDefault()
        event.stopPropagation()
        const selected = deps.getSelected()
        let idx = list.findIndex((entry) => selected.has(entry.path))
        if (idx < 0) idx = 0
        deps.setAnchorIndex(idx)
        deps.setCaretIndex(idx)
        void handleOpenEntry(list[idx])
        return
      }
    }

    const arrowNav = key === 'arrowdown' || key === 'arrowup'
    const arrowHoriz = key === 'arrowleft' || key === 'arrowright'
    if (
      (arrowNav || (arrowHoriz && deps.getViewMode() === 'grid')) &&
      !deps.isEditableTarget(event.target) &&
      rowsEl &&
      !inRows
    ) {
      const list = deps.getFilteredEntries()
      if (list.length > 0) {
        event.preventDefault()
        event.stopPropagation()
        rowsEl.focus()
        handleRowsKeydownCombined(event)
        return
      }
    }

    if (key === 'escape') {
      if (deps.isDeleteModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeDeleteModal()
        return
      }
      if (deps.isRenameModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeRenameModal()
        return
      }
      if (deps.isOpenWithModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeOpenWithModal()
        return
      }
      if (deps.isPropertiesModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closePropertiesModal()
        return
      }
      if (deps.isCompressModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeCompressModal()
        return
      }
      if (deps.isCheckDuplicatesModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeCheckDuplicatesModal()
        return
      }
      if (deps.isNewFolderModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeNewFolderModal()
        return
      }
      if (deps.isNewFileModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeNewFileModal()
        return
      }
      if (deps.isBookmarkModalOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeBookmarkModal()
        return
      }
      if (deps.isContextMenuOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeContextMenu()
        return
      }
      if (deps.isBlankMenuOpen()) {
        event.preventDefault()
        event.stopPropagation()
        deps.closeBlankContextMenu()
        return
      }
      if (blockingModalOpen) {
        return
      }
      if (deps.getMode() === 'filter') {
        event.preventDefault()
        event.stopPropagation()
        void deps.transitionToAddressMode({ path: deps.getCurrentPath(), blur: true })
        return
      }
      if (deps.isSearchSessionEnabled()) {
        event.preventDefault()
        event.stopPropagation()
        void deps.transitionToAddressMode({ path: deps.getCurrentPath(), blur: true })
        return
      }
      if (deps.isInputFocused() && deps.getMode() === 'address') {
        event.preventDefault()
        event.stopPropagation()
        deps.setPathInput(deps.getCurrentPath())
        deps.blurPathInput()
        return
      }
      if (inRows) {
        event.preventDefault()
        event.stopPropagation()
        clearSelection()
        return
      }
      if (deps.getSelected().size > 0) {
        clearSelection()
      }
    }

    if (blockingModalOpen) {
      return
    }
    void deps.handleGlobalKeydown(event)
  }

  const handleDocumentKeyup = (event: KeyboardEvent) => {
    if (event.key === 'Control' || event.key === 'Meta') {
      deps.setCopyModifierActive(false)
    }
  }

  return {
    handleDocumentKeydown,
    handleDocumentKeyup,
    handleOpenEntry,
    handleRowClickWithOpen,
    handleRowsMouseDown,
    handleRowsScrollCombined,
    handleRowsKeydownCombined,
    handleWheelCombined,
    handleRowsClickSafe,
    clearPendingOpenCandidate,
    cleanupScrollHover,
    clearSelection,
  }
}

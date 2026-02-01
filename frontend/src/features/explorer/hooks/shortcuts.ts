type ShortcutArgs = {
  isBookmarkModalOpen: () => boolean
  searchMode: () => boolean
  setSearchMode: (value: boolean) => Promise<void>
  focusPath: () => void
  blurPath: () => void
  onTypeChar: (char: string) => Promise<boolean> | boolean
  onRemoveChar: () => Promise<boolean> | boolean
  getSelectedPaths: () => string[]
  findEntryByPath: (path: string) => { kind: string } | null
  openBookmarkModal: (entry: { kind: string }) => Promise<void>
  goBack: () => void
  goForward: () => void
  onCopy?: () => Promise<boolean> | boolean
  onCut?: () => Promise<boolean> | boolean
  onPaste?: () => Promise<boolean> | boolean
  onRename?: () => Promise<boolean> | boolean
  onDelete?: (permanent: boolean) => Promise<boolean> | boolean
  onDeletePermanentFast?: () => Promise<boolean> | boolean
  onProperties?: () => Promise<boolean> | boolean
  onOpenConsole?: () => Promise<boolean> | boolean
  onToggleView?: () => Promise<void> | void
  onToggleHidden?: () => Promise<void> | void
}

export const createGlobalShortcuts = ({
  isBookmarkModalOpen,
  searchMode,
  setSearchMode,
  focusPath,
  blurPath,
  onTypeChar,
  onRemoveChar,
  getSelectedPaths,
  findEntryByPath,
  openBookmarkModal,
  goBack,
  goForward,
  onCopy,
  onCut,
  onPaste,
  onRename,
  onDelete,
  onDeletePermanentFast,
  onProperties,
  onOpenConsole,
  onToggleView,
  onToggleHidden,
}: ShortcutArgs) => {
  const isEditableTarget = (target: EventTarget | null) => {
    if (!(target instanceof HTMLElement)) return false
    const tag = target.tagName.toLowerCase()
    return (
      target.isContentEditable ||
      tag === 'input' ||
      tag === 'textarea' ||
      tag === 'select'
    )
  }

  const handleGlobalKeydown = async (event: KeyboardEvent) => {
    const key = typeof event.key === 'string' ? event.key.toLowerCase() : ''
    if (isBookmarkModalOpen()) return
    const editable = isEditableTarget(event.target)

    if (!event.ctrlKey && !event.metaKey && !event.altKey) {
      if (editable) return
      let char = ''
      let isDigit = false
      if (/^[a-z0-9]$/i.test(key)) {
        char = key
        isDigit = /^[0-9]$/.test(key)
      } else if (event.code?.startsWith('Digit')) {
        char = event.code.slice(5)
        isDigit = true
      } else if (event.code?.startsWith('Key')) {
        const base = event.code.slice(3)
        char = event.shiftKey ? base.toUpperCase() : base.toLowerCase()
      }

      if (char) {
        if (isDigit && event.shiftKey) {
          // Ignore Shift + digit so it doesn't trigger filtering.
          return
        }
        const handled = await onTypeChar(char)
        if (handled) {
          event.preventDefault()
          event.stopPropagation()
        }
        return
      }
    }

    if ((event.ctrlKey || event.metaKey) && key === 'f') {
      event.preventDefault()
      event.stopPropagation()
      if (!searchMode()) {
        await setSearchMode(true)
      }
      focusPath()
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'b') {
      event.preventDefault()
      event.stopPropagation()
      const selectedPaths = getSelectedPaths()
      if (selectedPaths.length === 1) {
        const entry = findEntryByPath(selectedPaths[0])
        if (entry && entry.kind === 'dir') {
          await openBookmarkModal(entry)
        }
      }
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'c') {
      if (isEditableTarget(event.target)) return
      if (onCopy) {
        await onCopy()
      }
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'x') {
      if (isEditableTarget(event.target)) return
      if (onCut) {
        await onCut()
      }
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'v') {
      if (isEditableTarget(event.target)) return
      if (onPaste) {
        await onPaste()
      }
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'p' && onProperties) {
      if (isEditableTarget(event.target)) return
      const handled = await onProperties()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 't' && onOpenConsole) {
      if (isEditableTarget(event.target)) return
      const handled = await onOpenConsole()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'g' && onToggleView) {
      await onToggleView()
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'h' && onToggleHidden) {
      await onToggleHidden()
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if (key === 'f2' && onRename) {
      if (isEditableTarget(event.target)) return
      const handled = await onRename()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (key === 'delete' && (onDeletePermanentFast || onDelete)) {
      if (isEditableTarget(event.target)) return
      if (event.shiftKey && onDeletePermanentFast) {
        const handled = await onDeletePermanentFast()
        if (handled) {
          event.preventDefault()
          event.stopPropagation()
        }
        return
      }
      const handled = await onDelete?.(false)
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (key === 'escape') {
      event.preventDefault()
      event.stopPropagation()
      if (searchMode()) {
        await setSearchMode(false)
        blurPath()
      }
      return
    }

    if (key === 'backspace') {
      if (event.ctrlKey || event.metaKey || event.altKey) return
      if (isEditableTarget(event.target)) return
      const handled = await onRemoveChar()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
      event.preventDefault()
      event.stopPropagation()
      if (event.shiftKey) {
        void goForward()
      } else {
        void goBack()
      }
    }
  }

  return { handleGlobalKeydown }
}

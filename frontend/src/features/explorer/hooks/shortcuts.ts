import type { ShortcutCommandId } from '../../shortcuts/keymap'

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
  isShortcut: (event: KeyboardEvent, commandId: ShortcutCommandId) => boolean
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
  onSelectAll?: () => Promise<boolean> | boolean
  onUndo?: () => Promise<boolean> | boolean
  onRedo?: () => Promise<boolean> | boolean
  onToggleSettings?: () => Promise<boolean> | boolean
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
  isShortcut,
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
  onSelectAll,
  onUndo,
  onRedo,
  onToggleSettings,
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

    if (isShortcut(event, 'open_settings') && onToggleSettings) {
      if (editable) return
      const handled = await onToggleSettings()
      if (handled !== false) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'search')) {
      if (editable) return
      event.preventDefault()
      event.stopPropagation()
      if (!searchMode()) {
        await setSearchMode(true)
      }
      focusPath()
      return
    }

    if (isShortcut(event, 'bookmarks')) {
      if (editable) return
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

    if (isShortcut(event, 'copy')) {
      if (editable) return
      if (onCopy) {
        await onCopy()
      }
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if (isShortcut(event, 'cut')) {
      if (editable) return
      if (onCut) {
        await onCut()
      }
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if (isShortcut(event, 'paste')) {
      if (editable) return
      if (onPaste) {
        await onPaste()
      }
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if (isShortcut(event, 'properties') && onProperties) {
      if (editable) return
      const handled = await onProperties()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'open_console') && onOpenConsole) {
      if (editable) return
      const handled = await onOpenConsole()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'toggle_view') && onToggleView) {
      if (editable) return
      await onToggleView()
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if (isShortcut(event, 'toggle_hidden') && onToggleHidden) {
      if (editable) return
      await onToggleHidden()
      event.preventDefault()
      event.stopPropagation()
      return
    }

    if (isShortcut(event, 'rename') && onRename) {
      if (editable) return
      const handled = await onRename()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'select_all') && onSelectAll) {
      if (editable) return
      const handled = await onSelectAll()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'undo') && onUndo) {
      if (editable) return
      const handled = await onUndo()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'redo') && onRedo) {
      if (editable) return
      const handled = await onRedo()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'delete_permanently') && onDeletePermanentFast) {
      if (editable) return
      const handled = await onDeletePermanentFast()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
      }
      return
    }

    if (isShortcut(event, 'delete_to_wastebasket') && onDelete) {
      if (editable) return
      const handled = await onDelete(false)
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

    const backShortcut = isShortcut(event, 'go_back')
    const forwardShortcut = isShortcut(event, 'go_forward')
    if (backShortcut || forwardShortcut) {
      if (editable) return
      const handled = await onRemoveChar()
      if (handled) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
      event.preventDefault()
      event.stopPropagation()
      if (forwardShortcut) {
        void goForward()
      } else {
        void goBack()
      }
    }
  }

  return { handleGlobalKeydown }
}

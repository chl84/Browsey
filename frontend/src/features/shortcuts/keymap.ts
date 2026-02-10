export type ShortcutCommandId =
  | 'search'
  | 'bookmarks'
  | 'copy'
  | 'cut'
  | 'paste'
  | 'toggle_view'
  | 'toggle_hidden'
  | 'open_settings'
  | 'open_console'
  | 'properties'
  | 'select_all'
  | 'undo'
  | 'redo'
  | 'delete_to_wastebasket'
  | 'delete_permanently'
  | 'rename'

export type ShortcutBinding = {
  commandId: ShortcutCommandId
  label: string
  context: string
  defaultAccelerator: string
  accelerator: string
}

type ParsedAccelerator = {
  ctrl: boolean
  alt: boolean
  shift: boolean
  key: string
}

export const DEFAULT_SHORTCUTS: ShortcutBinding[] = [
  { commandId: 'search', label: 'Search', context: 'global', defaultAccelerator: 'Ctrl+F', accelerator: 'Ctrl+F' },
  { commandId: 'bookmarks', label: 'Bookmarks', context: 'global', defaultAccelerator: 'Ctrl+B', accelerator: 'Ctrl+B' },
  { commandId: 'copy', label: 'Copy', context: 'global', defaultAccelerator: 'Ctrl+C', accelerator: 'Ctrl+C' },
  { commandId: 'cut', label: 'Cut', context: 'global', defaultAccelerator: 'Ctrl+X', accelerator: 'Ctrl+X' },
  { commandId: 'paste', label: 'Paste', context: 'global', defaultAccelerator: 'Ctrl+V', accelerator: 'Ctrl+V' },
  { commandId: 'toggle_view', label: 'Toggle view', context: 'global', defaultAccelerator: 'Ctrl+G', accelerator: 'Ctrl+G' },
  { commandId: 'toggle_hidden', label: 'Show hidden', context: 'global', defaultAccelerator: 'Ctrl+H', accelerator: 'Ctrl+H' },
  { commandId: 'open_settings', label: 'Open settings', context: 'global', defaultAccelerator: 'Ctrl+S', accelerator: 'Ctrl+S' },
  { commandId: 'open_console', label: 'Open console', context: 'global', defaultAccelerator: 'Ctrl+T', accelerator: 'Ctrl+T' },
  { commandId: 'properties', label: 'Properties', context: 'global', defaultAccelerator: 'Ctrl+P', accelerator: 'Ctrl+P' },
  { commandId: 'select_all', label: 'Select all', context: 'global', defaultAccelerator: 'Ctrl+A', accelerator: 'Ctrl+A' },
  { commandId: 'undo', label: 'Undo', context: 'global', defaultAccelerator: 'Ctrl+Z', accelerator: 'Ctrl+Z' },
  { commandId: 'redo', label: 'Redo', context: 'global', defaultAccelerator: 'Ctrl+Y', accelerator: 'Ctrl+Y' },
  { commandId: 'delete_to_wastebasket', label: 'Delete to wastebasket', context: 'global', defaultAccelerator: 'Delete', accelerator: 'Delete' },
  { commandId: 'delete_permanently', label: 'Delete permanently', context: 'global', defaultAccelerator: 'Shift+Delete', accelerator: 'Shift+Delete' },
  { commandId: 'rename', label: 'Rename', context: 'global', defaultAccelerator: 'F2', accelerator: 'F2' },
]

const normalizeKeyToken = (token: string): string | null => {
  const lowered = token.trim().toLowerCase()
  if (!lowered) return null
  if (lowered.length === 1 && /^[a-z0-9]$/.test(lowered)) return lowered
  if (/^f([1-9]|1[0-9]|2[0-4])$/.test(lowered)) return lowered
  switch (lowered) {
    case 'esc':
    case 'escape':
      return 'escape'
    case 'enter':
    case 'return':
      return 'enter'
    case 'tab':
      return 'tab'
    case 'space':
    case 'spacebar':
      return 'space'
    case 'backspace':
      return 'backspace'
    case 'delete':
    case 'del':
      return 'delete'
    case 'insert':
    case 'ins':
      return 'insert'
    case 'home':
      return 'home'
    case 'end':
      return 'end'
    case 'pageup':
    case 'pgup':
      return 'pageup'
    case 'pagedown':
    case 'pgdn':
      return 'pagedown'
    case 'arrowup':
    case 'up':
      return 'arrowup'
    case 'arrowdown':
    case 'down':
      return 'arrowdown'
    case 'arrowleft':
    case 'left':
      return 'arrowleft'
    case 'arrowright':
    case 'right':
      return 'arrowright'
    default:
      return null
  }
}

const displayKeyToken = (token: string): string => {
  if (token.length === 1) return token.toUpperCase()
  if (/^f[1-9][0-9]?$/.test(token) || /^f2[0-4]$/.test(token)) return token.toUpperCase()
  switch (token) {
    case 'escape':
      return 'Escape'
    case 'enter':
      return 'Enter'
    case 'tab':
      return 'Tab'
    case 'space':
      return 'Space'
    case 'backspace':
      return 'Backspace'
    case 'delete':
      return 'Delete'
    case 'insert':
      return 'Insert'
    case 'home':
      return 'Home'
    case 'end':
      return 'End'
    case 'pageup':
      return 'PageUp'
    case 'pagedown':
      return 'PageDown'
    case 'arrowup':
      return 'ArrowUp'
    case 'arrowdown':
      return 'ArrowDown'
    case 'arrowleft':
      return 'ArrowLeft'
    case 'arrowright':
      return 'ArrowRight'
    default:
      return token
  }
}

const parseAccelerator = (accelerator: string): ParsedAccelerator | null => {
  let ctrl = false
  let alt = false
  let shift = false
  let key: string | null = null

  const parts = accelerator.split('+').map((part) => part.trim()).filter(Boolean)
  if (parts.length === 0) return null

  for (const part of parts) {
    const lowered = part.toLowerCase()
    if (lowered === 'ctrl' || lowered === 'control' || lowered === 'cmd' || lowered === 'command' || lowered === 'meta') {
      ctrl = true
      continue
    }
    if (lowered === 'alt' || lowered === 'option') {
      alt = true
      continue
    }
    if (lowered === 'shift') {
      shift = true
      continue
    }
    if (key) return null
    key = normalizeKeyToken(part)
    if (!key) return null
  }

  if (!key) return null
  if (key.length === 1 && /^[a-z0-9]$/.test(key) && !ctrl && !alt) return null

  return { ctrl, alt, shift, key }
}

const eventToKeyToken = (event: KeyboardEvent): string | null => {
  const raw = event.key
  if (!raw) return null
  const lowered = raw.toLowerCase()
  if (lowered === 'control' || lowered === 'shift' || lowered === 'alt' || lowered === 'meta') {
    return null
  }
  if (lowered === ' ') return 'space'
  return normalizeKeyToken(raw)
}

export const keyboardEventToAccelerator = (event: KeyboardEvent): string | null => {
  const key = eventToKeyToken(event)
  if (!key) return null
  const ctrl = event.ctrlKey || event.metaKey
  const alt = event.altKey
  const shift = event.shiftKey

  if (key.length === 1 && /^[a-z0-9]$/.test(key) && !ctrl && !alt) {
    return null
  }

  const parts: string[] = []
  if (ctrl) parts.push('Ctrl')
  if (alt) parts.push('Alt')
  if (shift) parts.push('Shift')
  parts.push(displayKeyToken(key))
  return parts.join('+')
}

export const eventMatchesAccelerator = (event: KeyboardEvent, accelerator: string): boolean => {
  const parsed = parseAccelerator(accelerator)
  if (!parsed) return false
  const key = eventToKeyToken(event)
  if (!key) return false
  return (
    parsed.ctrl === (event.ctrlKey || event.metaKey) &&
    parsed.alt === event.altKey &&
    parsed.shift === event.shiftKey &&
    parsed.key === key
  )
}

export const matchesShortcut = (
  event: KeyboardEvent,
  shortcuts: ShortcutBinding[],
  commandId: ShortcutCommandId,
): boolean => {
  const found = shortcuts.find((shortcut) => shortcut.commandId === commandId)
  if (!found) return false
  return eventMatchesAccelerator(event, found.accelerator)
}

export const matchesAnyShortcut = (
  event: KeyboardEvent,
  shortcuts: ShortcutBinding[],
): boolean => {
  return shortcuts.some((shortcut) => eventMatchesAccelerator(event, shortcut.accelerator))
}

export const shortcutFor = (
  shortcuts: ShortcutBinding[],
  commandId: ShortcutCommandId,
): ShortcutBinding | null => {
  return shortcuts.find((shortcut) => shortcut.commandId === commandId) ?? null
}

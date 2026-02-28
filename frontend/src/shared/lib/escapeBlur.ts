const isTextInput = (target: EventTarget | null): target is HTMLInputElement => {
  if (!(target instanceof HTMLInputElement)) return false
  const type = (target.type || 'text').toLowerCase()
  return ['text', 'search', 'email', 'password', 'url', 'tel', 'number'].includes(type)
}

const isTextEntryTarget = (target: EventTarget | null): target is HTMLElement => {
  if (target instanceof HTMLTextAreaElement) return true
  if (target instanceof HTMLElement && target.isContentEditable) return true
  return isTextInput(target)
}

const resolveBlurTarget = (target: EventTarget | null): HTMLElement | null => {
  if (!isTextEntryTarget(target)) return null
  if (target instanceof HTMLElement && target.closest('.combo-search-wrap')) {
    return null
  }
  if (target instanceof HTMLElement && target.isContentEditable) {
    return target.closest<HTMLElement>('[contenteditable="true"]') ?? target
  }
  return target
}

export const blurTextEntryTargetOnEscape = (event: KeyboardEvent) => {
  if (event.key !== 'Escape') return false
  const blurTarget = resolveBlurTarget(event.target)
  if (!blurTarget) return false
  event.preventDefault()
  event.stopPropagation()
  blurTarget.blur()
  return true
}

import { writable } from 'svelte/store'

const isTextTarget = (target: EventTarget | null): target is HTMLElement => {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  if (target instanceof HTMLInputElement) {
    return !['button', 'submit', 'reset', 'checkbox', 'radio', 'file'].includes(target.type)
  }
  return target instanceof HTMLTextAreaElement
}

const targetReadonly = (target: HTMLElement) =>
  (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) &&
  (target.readOnly || target.disabled)

export const createTextContextMenu = () => {
  const open = writable(false)
  const x = writable(0)
  const y = writable(0)
  const target = writable<HTMLElement | null>(null)
  const readonly = writable(false)

  const close = () => {
    open.set(false)
    target.set(null)
    readonly.set(false)
  }

  const handleDocumentContextMenu = (event: MouseEvent) => {
    if (!isTextTarget(event.target)) return
    const nextTarget = event.target as HTMLElement
    event.preventDefault()
    event.stopPropagation()
    target.set(nextTarget)
    readonly.set(targetReadonly(nextTarget))
    open.set(true)
    x.set(event.clientX)
    y.set(event.clientY)
  }

  return {
    open,
    x,
    y,
    target,
    readonly,
    close,
    handleDocumentContextMenu,
  }
}

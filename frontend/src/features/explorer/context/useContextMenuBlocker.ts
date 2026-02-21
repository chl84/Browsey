import { onDestroy, onMount } from 'svelte'

const isFormField = (target: EventTarget | null) => {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName.toLowerCase()
  return target.isContentEditable || tag === 'input' || tag === 'textarea' || tag === 'select'
}

/**
 * Blocks native browser context menus globally, except inside form fields / contentEditable.
 * Returns nothing; lifecycle is tied to the calling component.
 */
export const useContextMenuBlocker = () => {
  let cleanup: (() => void) | null = null

  onMount(() => {
    const handler = (event: MouseEvent) => {
      if (!isFormField(event.target)) {
        event.preventDefault()
      }
    }
    window.addEventListener('contextmenu', handler, { capture: true })
    cleanup = () => window.removeEventListener('contextmenu', handler, { capture: true })
  })

  onDestroy(() => {
    cleanup?.()
  })
}

<script lang="ts">
  import { onDestroy, tick } from 'svelte'
  import { modalOpenState } from './modalOpenState'
  import { applyContainedWheelScrollAssist } from '../lib/wheelScrollAssist'
  import { blurTextEntryTargetOnEscape } from '../lib/escapeBlur'

  export let open = false
  export let title: string | null = null
  export let onClose: () => void = () => {}
  export let closeOnEscape = true
  export let closeOnOverlay = true
  export let guardOverlayPointer = true
  export let overlayClass = ''
  export let modalClass = ''
  export let modalWidth: string | null = null
  export let initialFocusSelector: string | null = null
  export let selectTextOnFocus = true

  let overlayPointerDown = false
  let modalEl: HTMLDivElement | null = null
  let countedAsOpen = false
  let restoreFocusTarget: HTMLElement | null = null

  const focusableSelectors = [
    'a[href]',
    'button:not([disabled])',
    'textarea:not([disabled])',
    'input:not([disabled]):not([type="hidden"])',
    'select:not([disabled])',
    '[tabindex]:not([tabindex="-1"])',
  ].join(', ')

  const getFocusableElements = () => {
    if (!modalEl) return []
    return Array.from(modalEl.querySelectorAll<HTMLElement>(focusableSelectors)).filter(
      (element) =>
        !element.hasAttribute('disabled') &&
        element.getAttribute('aria-hidden') !== 'true' &&
        element.getClientRects().length > 0,
    )
  }

  const trapTabFocus = (event: KeyboardEvent) => {
    const focusable = getFocusableElements()
    if (focusable.length === 0) {
      event.preventDefault()
      modalEl?.focus()
      return
    }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active =
      typeof document !== 'undefined' && document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null
    const activeInside = active ? modalEl?.contains(active) : false

    if (event.shiftKey) {
      if (!activeInside || active === first) {
        event.preventDefault()
        last.focus()
      }
      return
    }
    if (!activeInside || active === last) {
      event.preventDefault()
      first.focus()
    }
  }

  const captureRestoreFocusTarget = () => {
    if (typeof document === 'undefined') return
    const active = document.activeElement
    restoreFocusTarget = active instanceof HTMLElement ? active : null
  }

  const restoreFocusAfterClose = () => {
    const target = restoreFocusTarget
    restoreFocusTarget = null
    if (!target) return
    void tick().then(() => {
      if (open || !target.isConnected) return
      const active = typeof document !== 'undefined' ? document.activeElement : null
      if (active instanceof HTMLElement && active.isConnected && active !== document.body) {
        return
      }
      target.focus()
    })
  }

  $: {
    if (open && !countedAsOpen) {
      captureRestoreFocusTarget()
      modalOpenState.enter()
      countedAsOpen = true
    } else if (!open && countedAsOpen) {
      modalOpenState.leave()
      countedAsOpen = false
      restoreFocusAfterClose()
    }
  }

  onDestroy(() => {
    if (!countedAsOpen) return
    modalOpenState.leave()
    countedAsOpen = false
  })

  $: if (open) {
    void tick().then(() => {
      if (!open || !modalEl) return
      const focusTarget = initialFocusSelector
        ? modalEl.querySelector<HTMLElement>(initialFocusSelector) ?? modalEl
        : modalEl
      focusTarget.focus()
      if (
        selectTextOnFocus &&
        (focusTarget instanceof HTMLInputElement || focusTarget instanceof HTMLTextAreaElement)
      ) {
        focusTarget.select()
      }
    })
  }

  const handleOverlayPointerDown = (e: PointerEvent) => {
    if (!guardOverlayPointer) return
    overlayPointerDown = e.target === e.currentTarget
  }

  const handleOverlayClick = (e: MouseEvent) => {
    if (!closeOnOverlay) return
    if (!guardOverlayPointer || (overlayPointerDown && e.target === e.currentTarget)) {
      onClose()
    }
    overlayPointerDown = false
  }

  const handleKeydown = (e: KeyboardEvent) => {
    if (e.key === 'Tab') {
      trapTabFocus(e)
      return
    }
    if (blurTextEntryTargetOnEscape(e)) return
    if (!closeOnEscape) return
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      onClose()
    }
  }

  const handleWheel = (event: WheelEvent) => {
    if (!modalEl) return
    applyContainedWheelScrollAssist(modalEl, event)
  }
</script>

{#if open}
  <div
    class={`overlay ${overlayClass}`.trim()}
    role="presentation"
    tabindex="-1"
    on:pointerdown={handleOverlayPointerDown}
    on:click={handleOverlayClick}
    on:keydown={handleKeydown}
  >
    <div
      class={`modal ${modalClass}`.trim()}
      role="dialog"
      aria-modal="true"
      tabindex="0"
      style={modalWidth ? `--modal-width: ${modalWidth};` : undefined}
      on:click|stopPropagation
      on:keydown={handleKeydown}
      on:wheel={handleWheel}
      bind:this={modalEl}
    >
      {#if title || $$slots.header}
        <header>
          {#if $$slots.header}
            <slot name="header" />
          {:else}
            {title}
          {/if}
        </header>
      {/if}
      <slot />
      {#if $$slots.actions}
        <div class="actions">
          <slot name="actions" />
        </div>
      {/if}
    </div>
  </div>
{/if}

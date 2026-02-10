<script lang="ts">
  import { onDestroy, tick } from 'svelte'
  import { modalOpenState } from './modalOpenState'

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

  $: {
    if (open && !countedAsOpen) {
      modalOpenState.enter()
      countedAsOpen = true
    } else if (!open && countedAsOpen) {
      modalOpenState.leave()
      countedAsOpen = false
    }
  }

  onDestroy(() => {
    if (!countedAsOpen) return
    modalOpenState.leave()
    countedAsOpen = false
  })

  $: if (open && initialFocusSelector) {
    void tick().then(() => {
      if (!open || !modalEl) return
      const focusTarget = modalEl.querySelector<HTMLElement>(initialFocusSelector)
      if (!focusTarget) return
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
    if (!closeOnEscape) return
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
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

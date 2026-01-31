<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte'

  export type ComboOption = { value: string; label: string }

  export let options: ComboOption[] = []
  export let value: string = ''
  export let placeholder = 'Select'
  export let disabled = false

  const dispatch = createEventDispatcher<{ change: string }>()

  let open = false
  let highlighted = -1
  let rootEl: HTMLDivElement | null = null

  const currentIndex = () => options.findIndex((o) => o.value === value)

  $: {
    if (!open) {
      highlighted = currentIndex()
    } else if (highlighted < 0 && options.length > 0) {
      highlighted = 0
    }
  }

  const choose = (val: string) => {
    value = val
    dispatch('change', val)
    open = false
  }

  const onToggle = () => {
    if (disabled) return
    open = !open
  }

  const onOutside = (event: MouseEvent) => {
    if (!open || !rootEl) return
    if (!rootEl.contains(event.target as Node)) {
      open = false
    }
  }

  const move = (delta: number) => {
    if (!options.length) return
    const len = options.length
    highlighted = ((highlighted >= 0 ? highlighted : 0) + delta + len) % len
  }

  const handleKeydown = (e: KeyboardEvent) => {
    if (disabled) return
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (!open) open = true
      move(1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (!open) open = true
      move(-1)
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      if (!open) {
        open = true
      } else if (highlighted >= 0 && highlighted < options.length) {
        choose(options[highlighted].value)
      }
    } else if (e.key === 'Escape') {
      if (open) {
        e.preventDefault()
        open = false
      }
    }
  }

  onMount(() => {
    document.addEventListener('mousedown', onOutside, true)
  })

  onDestroy(() => {
    document.removeEventListener('mousedown', onOutside, true)
  })
</script>

<div
  class="combo"
  data-open={open}
  class:disabled={disabled}
  bind:this={rootEl}
>
  <button
    type="button"
    class="combo-btn"
    aria-haspopup="listbox"
    aria-expanded={open}
    disabled={disabled}
    on:click={onToggle}
    on:keydown={handleKeydown}
  >
    <span class="combo-label">
      {#if value && options.find((o) => o.value === value)}
        {options.find((o) => o.value === value)?.label}
      {:else}
        <span class="placeholder">{placeholder}</span>
      {/if}
    </span>
    <span class="chevron" aria-hidden="true">▾</span>
  </button>

  {#if open}
    <ul class="combo-list" role="listbox" tabindex="-1">
      {#each options as opt, i (opt.value)}
        <li
          role="option"
          aria-selected={opt.value === value}
          class:selected={opt.value === value}
          class:active={i === highlighted}
          on:mousedown={(e) => {
            e.preventDefault()
            choose(opt.value)
          }}
          on:mousemove={() => (highlighted = i)}
        >
          {opt.label}
        </li>
      {/each}
    </ul>
  {/if}
</div>

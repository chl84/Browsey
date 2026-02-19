<script lang="ts" context="module">
  export type ComboOption = { value: string; label: string }
</script>

<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount, tick } from 'svelte'

  export let options: ComboOption[] = []
  export let value: string = ''
  export let placeholder = 'Select'
  export let disabled = false
  export let searchable = false
  export let searchPlaceholder = 'Search…'
  export let emptyLabel = 'No options available'
  export let noMatchesLabel = 'No matches'

  const dispatch = createEventDispatcher<{ change: string }>()

  let open = false
  let highlighted = -1
  let rootEl: HTMLDivElement | null = null
  let searchInputEl: HTMLInputElement | null = null
  let searchQuery = ''
  let filteredOptions: ComboOption[] = []
  let selectedOption: ComboOption | undefined

  $: selectedOption = options.find((o) => o.value === value)
  $: {
    const needle = searchable ? searchQuery.trim().toLowerCase() : ''
    filteredOptions =
      needle.length === 0
        ? options
        : options.filter(
            (o) =>
              o.label.toLowerCase().includes(needle) || o.value.toLowerCase().includes(needle),
          )
  }

  const currentIndex = () => filteredOptions.findIndex((o) => o.value === value)

  $: {
    if (!open) {
      highlighted = currentIndex()
    } else if (highlighted < 0 && filteredOptions.length > 0) {
      highlighted = currentIndex()
      if (highlighted < 0) highlighted = 0
    } else if (highlighted >= filteredOptions.length) {
      highlighted = filteredOptions.length - 1
    }
  }

  const focusSearchInput = () => {
    if (!searchable) return
    void tick().then(() => searchInputEl?.focus())
  }

  const openDropdown = () => {
    if (disabled) return
    searchQuery = ''
    open = true
    highlighted = currentIndex()
    focusSearchInput()
  }

  const closeDropdown = () => {
    open = false
    searchQuery = ''
  }

  const choose = (val: string) => {
    value = val
    dispatch('change', val)
    closeDropdown()
  }

  const onToggle = () => {
    if (disabled) return
    if (open) {
      closeDropdown()
    } else {
      openDropdown()
    }
  }

  const onOutside = (event: MouseEvent) => {
    if (!open || !rootEl) return
    if (!rootEl.contains(event.target as Node)) {
      closeDropdown()
    }
  }

  const move = (delta: number) => {
    if (!filteredOptions.length) return
    const len = filteredOptions.length
    highlighted = ((highlighted >= 0 ? highlighted : 0) + delta + len) % len
  }

  const handleKeydown = (e: KeyboardEvent) => {
    if (disabled) return
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (!open) openDropdown()
      move(1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (!open) openDropdown()
      move(-1)
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      if (!open) {
        openDropdown()
      } else if (highlighted >= 0 && highlighted < filteredOptions.length) {
        choose(filteredOptions[highlighted].value)
      }
    } else if (e.key === 'Escape') {
      if (open) {
        e.preventDefault()
        closeDropdown()
      }
    }
  }

  const handleSearchKeydown = (e: KeyboardEvent) => {
    if (!open) return
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      move(1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      move(-1)
    } else if (e.key === 'Enter') {
      if (highlighted >= 0 && highlighted < filteredOptions.length) {
        e.preventDefault()
        choose(filteredOptions[highlighted].value)
      }
    } else if (e.key === 'Escape') {
      e.preventDefault()
      closeDropdown()
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
      {#if value && selectedOption}
        {selectedOption.label}
      {:else}
        <span class="placeholder">{placeholder}</span>
      {/if}
    </span>
    <span class="chevron" aria-hidden="true">▾</span>
  </button>

  {#if open}
    <div class="combo-list-wrap">
      {#if searchable}
        <div class="combo-search-wrap">
          <input
            class="combo-search"
            type="text"
            bind:value={searchQuery}
            bind:this={searchInputEl}
            placeholder={searchPlaceholder}
            on:keydown={handleSearchKeydown}
          />
        </div>
      {/if}

      <ul class="combo-list" role="listbox" tabindex="-1">
        {#if filteredOptions.length === 0}
          <li class="empty">
            {searchable && searchQuery.trim().length > 0 ? noMatchesLabel : emptyLabel}
          </li>
        {:else}
          {#each filteredOptions as opt, i (opt.value)}
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
        {/if}
      </ul>
    </div>
  {/if}
</div>

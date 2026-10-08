<script lang="ts" context="module">
  export type ComboOption = { value: string; label: string; disabled?: boolean }
  let nextListId = 0
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
  export let ariaLabel: string | undefined = undefined
  export let resetOnSelect = false
  export let fixedDropdown = false

  const dispatch = createEventDispatcher<{ change: string }>()

  let open = false
  let highlighted = -1
  let rootEl: HTMLDivElement | null = null
  let triggerEl: HTMLButtonElement | null = null
  let searchInputEl: HTMLInputElement | null = null
  let searchQuery = ''
  let filteredOptions: ComboOption[] = []
  let selectedOption: ComboOption | undefined
  let listWrapEl: HTMLDivElement | null = null
  let listEl: HTMLUListElement | null = null
  let openDirection: 'down' | 'up' = 'down'
  let listMaxHeight = 240
  const listId = `combo-list-${++nextListId}`
  let popupLeft = 0
  let popupTop = 0
  let popupWidth = 240
  let popupReady = false

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
  const firstEnabledIndex = () => filteredOptions.findIndex((o) => !o.disabled)

  $: {
    if (!open) {
      highlighted = currentIndex()
    } else if (highlighted < 0 || highlighted >= filteredOptions.length || filteredOptions[highlighted]?.disabled) {
      highlighted = currentIndex()
      if (highlighted < 0 || filteredOptions[highlighted]?.disabled) highlighted = firstEnabledIndex()
    }
  }

  const revealHighlightedOption = () => {
    if (!open || !listEl) return
    const option = listEl.querySelector<HTMLElement>('.active')
    if (!option) return
    const listTop = listEl.getBoundingClientRect().top + listEl.clientTop
    const listBottom = listTop + listEl.clientHeight
    const optionRect = option.getBoundingClientRect()
    // Scroll only the option list, preserving the dialog and page position.
    if (optionRect.top < listTop) {
      listEl.scrollTop += optionRect.top - listTop
    } else if (optionRect.bottom > listBottom) {
      listEl.scrollTop += optionRect.bottom - listBottom
    }
  }

  $: if (open && highlighted >= 0 && filteredOptions.length > 0 && listMaxHeight > 0) {
    void tick().then(revealHighlightedOption)
  }

  const focusSearchInput = () => {
    if (!searchable) return
    void tick().then(() => searchInputEl?.focus())
  }

  const updateDropdownPlacement = async () => {
    if (!open || !rootEl || !listWrapEl) return
    const triggerRect = rootEl.getBoundingClientRect()
    if (fixedDropdown) {
      popupWidth = Math.min(Math.max(triggerRect.width, 240), window.innerWidth - 16)
      popupLeft = Math.max(8, Math.min(triggerRect.right - popupWidth, window.innerWidth - popupWidth - 8))
      await tick()
      if (!open || !listWrapEl) return
    }
    const wrapRect = listWrapEl.getBoundingClientRect()
    const viewportHeight = window.innerHeight
    const gap = 2
    const searchHeight = searchable ? 42 : 0
    const minListHeight = 80
    const modalRect = fixedDropdown ? rootEl.closest('.modal')?.getBoundingClientRect() : undefined
    const spaceAbove = Math.max(0, triggerRect.top - Math.max(0, modalRect?.top ?? 0) - gap)
    const spaceBelow = Math.max(0, Math.min(viewportHeight, modalRect?.bottom ?? viewportHeight) - triggerRect.bottom - gap)
    const preferredDirection =
      spaceBelow >= wrapRect.height || spaceBelow >= spaceAbove ? 'down' : 'up'

    openDirection = preferredDirection
    const availableSpace = preferredDirection === 'down' ? spaceBelow : spaceAbove
    listMaxHeight = Math.max(minListHeight, Math.min(240, availableSpace - searchHeight - 8))
    if (fixedDropdown) {
      await tick()
      if (!open || !listWrapEl) return
      popupTop = preferredDirection === 'down' ? triggerRect.bottom + gap
        : triggerRect.top - listWrapEl.getBoundingClientRect().height - gap
      popupReady = true
    }
  }

  const openDropdown = () => {
    if (disabled) return
    searchQuery = ''
    open = true
    popupReady = false
    highlighted = currentIndex()
    focusSearchInput()
    void tick().then(() => updateDropdownPlacement())
  }

  const closeDropdown = () => {
    open = false
    searchQuery = ''
  }
  $: if (disabled && open) closeDropdown()

  const dismissDropdown = (event: KeyboardEvent) => {
    event.preventDefault()
    event.stopPropagation()
    closeDropdown()
    triggerEl?.focus()
  }

  const choose = (val: string) => {
    if (disabled || filteredOptions.find(o => o.value === val)?.disabled) return
    value = resetOnSelect ? '' : val
    closeDropdown()
    if (resetOnSelect) triggerEl?.focus()
    dispatch('change', val)
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
    let index = highlighted >= 0 ? highlighted : delta > 0 ? -1 : 0
    for (let count = 0; count < len; count++) {
      index = (index + delta + len) % len
      if (!filteredOptions[index].disabled) { highlighted = index; return }
    }
    highlighted = -1
  }

  const handleKeydown = (e: KeyboardEvent) => {
    if (disabled) return
    if (e.key === 'Tab') {
      closeDropdown()
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (!open && currentIndex() < 0) openDropdown()
      else { if (!open) openDropdown(); move(1) }
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
        dismissDropdown(e)
      }
    }
  }

  const handleScroll = (event: Event) => {
    if (!open || !rootEl) return
    if (fixedDropdown && event.target instanceof Node && !listWrapEl?.contains(event.target)) {
      // Browser focus may scroll the trigger just after opening. Reposition
      // while it remains visible; dismiss only when its anchor scrolls away.
      const trigger = rootEl.getBoundingClientRect()
      for (let parent = rootEl.parentElement; parent; parent = parent.parentElement) {
        if (!/(auto|scroll|hidden|clip)/.test(getComputedStyle(parent).overflowY)) continue
        const bounds = parent.getBoundingClientRect()
        if (trigger.bottom <= bounds.top || trigger.top >= bounds.bottom) { closeDropdown(); return }
      }
    }
    void updateDropdownPlacement()
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
      dismissDropdown(e)
    }
  }

  onMount(() => {
    document.addEventListener('mousedown', onOutside, true)
    window.addEventListener('resize', updateDropdownPlacement)
    window.addEventListener('scroll', handleScroll, true)
  })

  onDestroy(() => {
    document.removeEventListener('mousedown', onOutside, true)
    window.removeEventListener('resize', updateDropdownPlacement)
    window.removeEventListener('scroll', handleScroll, true)
  })
</script>

<div
  class="combo"
  data-open={open}
  data-direction={openDirection}
  class:disabled={disabled}
  bind:this={rootEl}
>
  <button
    type="button"
    class="combo-btn"
    role={resetOnSelect ? 'combobox' : undefined}
    aria-label={ariaLabel}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? listId : undefined}
    aria-activedescendant={resetOnSelect && open && highlighted >= 0 ? `${listId}-${highlighted}` : undefined}
    bind:this={triggerEl}
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
    <div class="combo-list-wrap" bind:this={listWrapEl}
      style={fixedDropdown ? `position: fixed; left: ${popupLeft}px; top: ${popupTop}px; width: ${popupWidth}px; right: auto; bottom: auto; visibility: ${popupReady ? 'visible' : 'hidden'};` : undefined}>
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

      <ul id={listId} class="combo-list" role="listbox" tabindex="-1" bind:this={listEl} style={`max-height: ${listMaxHeight}px;`}>
        {#if filteredOptions.length === 0}
          <li class="empty">
            {searchable && searchQuery.trim().length > 0 ? noMatchesLabel : emptyLabel}
          </li>
        {:else}
          {#each filteredOptions as opt, i (opt.value)}
            <li
              role="option"
              id={`${listId}-${i}`}
              aria-selected={opt.value === value}
              aria-disabled={opt.disabled || undefined}
              class:disabled={opt.disabled}
              class:selected={opt.value === value}
              class:active={i === highlighted}
              on:mousedown={(e) => {
                e.preventDefault()
                choose(opt.value)
              }}
              on:mousemove={() => { if (!opt.disabled) highlighted = i }}
            >
              {opt.label}
            </li>
          {/each}
        {/if}
      </ul>
    </div>
  {/if}
</div>

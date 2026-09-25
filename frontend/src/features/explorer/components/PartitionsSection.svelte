<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { partitionIcon } from '../utils'
  import { fullNameTooltip } from '../helpers/fullNameTooltip'
  import type { Partition } from '../model/types'
  import ContextMenu from './ContextMenu.svelte'
  import { canFormatPartition, isMtpPartition, isUnmountedPartition } from '../services/drives.service'

  const dispatch = createEventDispatcher<{ eject: { path: string }; format: { part: Partition }; properties: { part: Partition } }>()

  export let partitions: Partition[] = []
  export let onSelect: (path: string) => void = () => {}

  const eject = (path: string) => dispatch('eject', { path })
  let menu = { open: false, x: 0, y: 0, part: null as Partition | null }
  const openMenu = (event: MouseEvent, part: Partition) => {
    if (!part.removable) return
    event.preventDefault()
    ;(event.currentTarget as HTMLElement).focus()
    menu = { open: true, x: event.clientX, y: event.clientY, part }
  }
  const openMenuFromButton = (button: HTMLElement, part: Partition) => {
    if (!part.removable) return
    const rect = button.getBoundingClientRect()
    menu = { open: true, x: rect.right, y: rect.bottom, part }
  }
</script>

<div class="section">
  <div class="section-title">Partitions</div>
  {#each partitions as part}
    <div class="row">
      <button
        class="nav"
        type="button"
        on:click={() => onSelect(part.path)}
        on:contextmenu={(e) => openMenu(e, part)}
        on:keydown={(event) => {
          if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
            event.preventDefault()
            openMenuFromButton(event.currentTarget, part)
          }
        }}
      >
        {#if isMtpPartition(part)}
          <svg class="nav-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
            <rect x="5" y="2" width="10" height="16" rx="2" />
            <path d="M8 5h4M9 15h2" />
          </svg>
        {:else}
          <img class="nav-icon" src={partitionIcon(part)} alt="" />
        {/if}
        <span class="nav-label">{part.label}{isUnmountedPartition(part.path) ? ' (not mounted)' : ''}</span>
      </button>
      {#if part.removable}
        <button
          class="more"
          type="button"
          aria-label={isMtpPartition(part) ? 'Phone actions' : 'USB actions'}
          use:fullNameTooltip={isMtpPartition(part) ? 'Phone actions' : 'USB actions'}
          on:click={(event) => openMenuFromButton(event.currentTarget, part)}
        >
          <span aria-hidden="true">⋮</span>
        </button>
        {#if !isUnmountedPartition(part.path)}<button
          class="eject"
          type="button"
          aria-label="Eject"
          use:fullNameTooltip={'Eject'}
          on:click={() => eject(part.path)}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path
              d="M10 4.5 3.5 12h13L10 4.5Zm-6 10h12v2H4v-2Z"
              fill="currentColor"
              fill-rule="evenodd"
              clip-rule="evenodd" />
          </svg>
        </button>{/if}
      {/if}
    </div>
  {/each}
</div>

<ContextMenu
  open={menu.open}
  x={menu.x}
  y={menu.y}
  actions={[
    ...(menu.part && isUnmountedPartition(menu.part.path) ? [{ id: 'mount', label: 'Mount and open' }] : []),
    ...(menu.part && canFormatPartition(menu.part) ? [{ id: 'format', label: 'Format…', dangerous: true }] : []),
    { id: 'properties', label: 'Properties' },
  ]}
  onClose={() => (menu = { ...menu, open: false })}
  onSelect={(id) => {
    if (id === 'format' && menu.part && canFormatPartition(menu.part)) dispatch('format', { part: menu.part })
    if (id === 'properties' && menu.part) dispatch('properties', { part: menu.part })
    if (id === 'mount' && menu.part) onSelect(menu.part.path)
    menu = { ...menu, open: false }
  }}
/>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-title {
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
    padding-left: 10px;
  }

  .row {
    position: relative;
    width: 100%;
  }

  .nav {
    border: none;
    border-radius: 0;
    padding: 5px 66px 5px 22px; /* extra right padding so hover bg reaches behind controls */
    background: transparent;
    color: var(--fg);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    width: 100%;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
    text-align: left;
  }

  .nav:hover {
    background: var(--bg-hover);
  }

  .nav:focus-visible {
    outline: 2px solid var(--border-accent);
    outline-offset: -2px;
  }

  .nav:active {
    transform: none;
    box-shadow: none;
  }

  .nav-icon {
    width: 18px;
    height: 18px;
    object-fit: contain;
    flex-shrink: 0;
  }

  .eject {
    position: absolute;
    top: 50%;
    right: 2px;
    transform: translateY(-50%);
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 4px 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: color 120ms ease;
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
  }

  .more {
    position: absolute;
    top: 50%;
    right: 28px;
    transform: translateY(-50%);
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 2px 6px;
    min-width: 22px;
    line-height: 1;
    font-size: 18px;
    cursor: pointer;
  }

  .more:hover,
  .more:focus-visible {
    color: var(--fg);
  }

  .eject:hover {
    color: var(--fg);
  }

  .eject svg {
    width: 16px;
    height: 16px;
  }
</style>

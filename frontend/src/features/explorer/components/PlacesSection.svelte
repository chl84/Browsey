<script lang="ts">
  import { onMount } from 'svelte'
  import { getHomeDirectory } from '../services/files.service'
  import { navIcon } from '../utils'
  import ContextMenu from './ContextMenu.svelte'

  export let places: { label: string; path: string }[] = []
  export let onSelect: (label: string, path: string) => void = () => {}
  export let onEmptyWastebasket: () => void = () => {}
  let menu = { open: false, x: 0, y: 0 }
  const openMenu = (event: MouseEvent | KeyboardEvent, path: string) => {
    if (path !== 'trash://') return
    if (event instanceof KeyboardEvent && event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
    event.preventDefault()
    const trigger = event.currentTarget as HTMLButtonElement
    trigger.focus()
    const rect = trigger.getBoundingClientRect()
    menu = { open: true, x: event instanceof MouseEvent ? event.clientX : rect.right, y: event instanceof MouseEvent ? event.clientY : rect.bottom }
  }
  let homeDropPath = ''
  onMount(() => {
    let active = true
    void getHomeDirectory().then(path => { if (active) homeDropPath = path }).catch(() => {
      // Leave Home unavailable as a drop target if its absolute path cannot be resolved.
    })
    return () => { active = false }
  })
</script>

<div class="section-wrapper">
  <div class="section">
    <div class="section-title">Places</div>
    {#each places as place}
      <button class="nav" data-drop-path={place.path === '~' ? homeDropPath : place.path} type="button" on:click={() => onSelect(place.label, place.path)}
        on:contextmenu={(event) => openMenu(event, place.path)}
        on:keydown={(event) => openMenu(event, place.path)}>
        <img class="nav-icon" src={navIcon(place.label)} alt="" />
        <span class="nav-label">{place.label}</span>
      </button>
    {/each}
  </div>
</div>

<ContextMenu
  open={menu.open}
  x={menu.x}
  y={menu.y}
  actions={[{ id: 'empty-wastebasket', label: 'Empty Wastebasket…', dangerous: true }]}
  onClose={() => { menu.open = false }}
  onSelect={(id) => {
    menu.open = false
    if (id === 'empty-wastebasket') onEmptyWastebasket()
  }}
/>

<style>
  .section-wrapper {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

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

  .nav {
    border: none;
    border-radius: 0;
    padding: calc((var(--sidebar-row-height) - 18px) / 2) 22px;
    background: transparent;
    color: var(--fg);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
  }

  .nav:hover {
    background: var(--bg-hover);
    transform: none;
    box-shadow: none;
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

</style>
